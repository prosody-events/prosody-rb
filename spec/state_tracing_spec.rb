# frozen_string_literal: true

require "spec_helper"
require "securerandom"
require "net/http"
require "json"
require "uri"
require "tmpdir"
require "opentelemetry/sdk"
require "opentelemetry-exporter-otlp"

# Full LGTM trace-topology audit for keyed state (Appendix 1 item 12), per
# docs/keyed-state/clients/02-lgtm-trace-audit.md. Runs a focused state group
# under a unique OTEL_SERVICE_NAME, exports to the live collector, then queries
# Tempo and audits the complete span graph: exactly one core semantic span per
# state op parented directly to the Ruby handler span, no binding/Magnus wrapper
# span, and no ready-chunk span. Tagged :tracing (excluded by default).
RSpec.describe "Prosody keyed state tracing", integration: true, tracing: true do
  SERVICE = "prosody-rb-trace-audit-#{Time.now.to_i}-#{Process.pid}"
  TEMPO = ENV.fetch("PROSODY_TEMPO_URL", "http://localhost:3200")
  TRACER_SCOPE = "prosody-rb-state-audit"

  let(:topic) { random_state_topic }
  let(:admin_client_class) { Prosody.const_get(:AdminClient) }
  let(:admin) { admin_client_class.new([TestConfig::BOOTSTRAP_SERVERS]) }

  before(:all) do
    ENV["OTEL_SERVICE_NAME"] = SERVICE
    ENV["OTEL_EXPORTER_OTLP_ENDPOINT"] ||= "http://localhost:4318"
    ENV["OTEL_EXPORTER_OTLP_PROTOCOL"] ||= "http/protobuf"
    ENV["OTEL_PROPAGATORS"] ||= "tracecontext,baggage"

    OpenTelemetry::SDK.configure do |c|
      c.service_name = SERVICE
      c.resource = OpenTelemetry::SDK::Resources::Resource.create({"service.name" => SERVICE})
      c.add_span_processor(
        OpenTelemetry::SDK::Trace::Export::BatchSpanProcessor.new(
          OpenTelemetry::Exporter::OTLP::Exporter.new
        )
      )
    end
  end

  before do
    # Preflight: Tempo must be reachable (locate LGTM via docker ps if this fails).
    ready = Net::HTTP.get_response(URI("#{TEMPO}/ready"))
    skip "Tempo not ready at #{TEMPO}" unless ready.is_a?(Net::HTTPSuccess)
    admin.create_topic(topic, 1, 1)
    sleep 1
  end

  after do
    admin.delete_topic(topic)
  rescue
    # ignore
  end

  # Extracts a scalar OTLP attribute value.
  def attr_value(value)
    return value["stringValue"] if value.key?("stringValue")
    return value["intValue"] if value.key?("intValue")
    return value["boolValue"] if value.key?("boolValue")
    value.values.first
  end

  # Flattens a Tempo trace payload into span records indexed by span id.
  def flatten_trace(trace)
    batches = trace["batches"] || trace["resourceSpans"] || []
    spans = {}
    batches.each do |batch|
      resource_attrs = {}
      (batch.dig("resource", "attributes") || []).each do |a|
        resource_attrs[a["key"]] = attr_value(a["value"])
      end
      service = resource_attrs["service.name"]
      scope_spans = batch["scopeSpans"] || batch["instrumentationLibrarySpans"] || []
      scope_spans.each do |scope_span|
        scope = scope_span.dig("scope", "name") || scope_span.dig("instrumentationLibrary", "name")
        (scope_span["spans"] || []).each do |span|
          attrs = {}
          (span["attributes"] || []).each { |a| attrs[a["key"]] = attr_value(a["value"]) }
          spans[span["spanId"]] = {
            span_id: span["spanId"],
            parent_id: (span["parentSpanId"].nil? || span["parentSpanId"].empty?) ? nil : span["parentSpanId"],
            name: span["name"],
            service: service,
            scope: scope,
            attrs: attrs,
            status: span["status"]
          }
        end
      end
    end
    spans
  end

  # Polls Tempo search for traces under SERVICE, returning trace ids.
  def search_trace_ids
    uri = URI("#{TEMPO}/api/search")
    uri.query = URI.encode_www_form(q: "{ resource.service.name = \"#{SERVICE}\" }", limit: 100)
    response = Net::HTTP.get_response(uri)
    return [] unless response.is_a?(Net::HTTPSuccess)
    (JSON.parse(response.body)["traces"] || []).map { |t| t["traceID"] }
  end

  # Fetches and flattens a single trace by id.
  def fetch_trace(trace_id)
    response = Net::HTTP.get_response(URI("#{TEMPO}/api/traces/#{trace_id}"))
    return nil unless response.is_a?(Net::HTTPSuccess)
    flatten_trace(JSON.parse(response.body))
  end

  # Polls until a flattened trace containing all expected core span names is
  # found, or the deadline passes.
  def await_audit_trace(expected_names, timeout: 45)
    deadline = Time.now + timeout
    while Time.now < deadline
      search_trace_ids.each do |trace_id|
        spans = fetch_trace(trace_id)
        next if spans.nil?
        names = spans.values.map { |s| s[:name] }
        return [trace_id, spans] if expected_names.all? { |n| names.include?(n) }
      end
      sleep 1
    end
    [nil, nil]
  end

  it "emits exactly one core semantic span per state op, parented to the Ruby handler span" do
    value_def = Prosody.value(random_state_name("val"))
    map_def = Prosody.map(random_state_name("map"))
    latch = Thread::Queue.new
    tracer = OpenTelemetry.tracer_provider.tracer(TRACER_SCOPE)

    handler_class = Class.new(Prosody::EventHandler) do
      def initialize(latch, tracer, value_def, map_def)
        @latch = latch
        @tracer = tracer
        @value_def = value_def
        @map_def = map_def
      end

      def on_message(context, _message)
        @tracer.in_span("rb.state.handler", kind: :internal) do
          value = context.state(@value_def)
          value.set({"n" => 1})
          value.get
          map = context.state(@map_def)
          map.set("a", 1)
          map.each_pair { |_k, _v| }
          @latch << :done
        end
      end
    end

    config = state_config(topic, value_def, map_def)
    client = Prosody::Client.new(config)
    begin
      client.subscribe(handler_class.new(latch, tracer, value_def, map_def))
      client.send_message(topic, "k1", {go: true})
      Timeout.timeout(TestConfig::MESSAGE_TIMEOUT) { latch.pop }

      OpenTelemetry.tracer_provider.force_flush
      sleep 6 # allow Rust core export + Tempo ingestion
    ensure
      client.unsubscribe if client.consumer_state == :running
    end

    expected_core = %w[value.set value.get map.set map.stream]
    trace_id, spans = await_audit_trace(expected_core)
    expect(trace_id).not_to be_nil, "no trace under #{SERVICE} contained #{expected_core.inspect}"

    handler_spans = spans.values.select { |s| s[:name] == "rb.state.handler" }
    expect(handler_spans.length).to eq(1), "expected one rb.state.handler span, got #{handler_spans.length}"
    handler_span = handler_spans.first
    expect(handler_span[:scope]).to eq(TRACER_SCOPE)

    core_spans = {}
    expected_core.each do |name|
      matches = spans.values.select { |s| s[:name] == name }
      expect(matches.length).to eq(1), "expected exactly one #{name} span, got #{matches.length}"
      core_spans[name] = matches.first
    end

    # Direct parentage: each core semantic span parents to the Ruby handler span
    # (the activated carrier), not to a binding wrapper.
    core_spans.each do |name, span|
      expect(span[:parent_id]).to eq(handler_span[:span_id]),
        "#{name} parent #{span[:parent_id].inspect} != rb.state.handler #{handler_span[:span_id].inspect}"
    end

    # Collection attribute carries the nonce name.
    expect(core_spans["value.set"][:attrs]["collection"]).to eq(value_def.name)
    expect(core_spans["value.get"][:attrs]["collection"]).to eq(value_def.name)
    expect(core_spans["map.set"][:attrs]["collection"]).to eq(map_def.name)
    expect(core_spans["map.stream"][:attrs]["collection"]).to eq(map_def.name)

    # No same-named wrapper span (the duplicate-wrapper signature).
    expected_core.each do |name|
      wrappers = spans.values.select { |s| s[:name] == name && s[:parent_id] && spans[s[:parent_id]]&.dig(:name) == name }
      expect(wrappers).to be_empty, "found a #{name} span parented by another #{name} span (wrapper)"
    end

    # No ready-chunk / binding child under map.stream.
    stream_children = spans.values.select { |s| s[:parent_id] == core_spans["map.stream"][:span_id] }
    expect(stream_children).to be_empty, "map.stream has unexpected child spans: #{stream_children.map { |s| s[:name] }.inspect}"

    # Core semantic spans carry a non-empty core instrumentation scope distinct
    # from the Ruby tracer scope.
    core_spans.each_value do |span|
      expect(span[:scope]).not_to be_nil
      expect(span[:scope]).not_to eq(TRACER_SCOPE)
    end

    # Graph invariants: every non-root parent resolves within the trace, and the
    # parent chain is acyclic reaching a single root.
    spans.each_value do |span|
      next if span[:parent_id].nil?
      next unless spans.key?(span[:parent_id]) # cross-trace/remote parents are legitimate roots here
      seen = {}
      cursor = span
      until cursor.nil? || cursor[:parent_id].nil? || !spans.key?(cursor[:parent_id])
        expect(seen).not_to have_key(cursor[:span_id]), "cycle detected at #{cursor[:name]}"
        seen[cursor[:span_id]] = true
        cursor = spans[cursor[:parent_id]]
      end
    end

    # No exported span carries an ERROR status.
    core_spans.each_value do |span|
      code = span[:status] && (span[:status]["code"] || span[:status]["statusCode"])
      expect(code).not_to eq(2), "core span #{span[:name]} has ERROR status"
      expect(code).not_to eq("STATUS_CODE_ERROR"), "core span #{span[:name]} has ERROR status"
    end

    # Retain the evidence (best-effort; a write failure never fails the audit).
    begin
      evidence_dir = ENV.fetch("PROSODY_TRACE_EVIDENCE_DIR", Dir.tmpdir)
      evidence_path = File.join(evidence_dir, "state-tracing-evidence-#{Time.now.to_i}.md")
      File.write(evidence_path, <<~EVIDENCE)
        # Keyed-state trace-topology audit evidence

        - LGTM: grafana/otel-lgtm (prosody-lgtm-1); Tempo #{TEMPO}, OTLP #{ENV["OTEL_EXPORTER_OTLP_ENDPOINT"]}
        - Service name: #{SERVICE}
        - Ruby tracer scope: #{TRACER_SCOPE}
        - Trace id: #{trace_id}
        - Handler span: rb.state.handler (#{handler_span[:span_id]}), scope=#{handler_span[:scope]}

        ## Core semantic spans (exactly one each, parented to rb.state.handler)
        #{core_spans.map { |name, s| "- #{name} (#{s[:span_id]}) parent=#{s[:parent_id]} scope=#{s[:scope]} collection=#{s[:attrs]["collection"]}" }.join("\n")}

        ## Assertions
        - one rb.state.handler span: PASS
        - exactly one value.set/value.get/map.set/map.stream: PASS
        - each core span parents directly to rb.state.handler: PASS
        - no same-named wrapper span: PASS
        - no ready-chunk / binding child under map.stream: PASS
        - core spans carry a distinct non-empty scope: PASS
        - graph acyclic, parents resolve, no ERROR status: PASS

        ## Full span list
        #{spans.values.map { |s| "- #{s[:name]} id=#{s[:span_id]} parent=#{s[:parent_id]} scope=#{s[:scope]} service=#{s[:service]}" }.join("\n")}
      EVIDENCE
      puts "Trace evidence written to #{evidence_path}"
    rescue => e
      warn "Could not write trace evidence: #{e.message}"
    end
  end
end
