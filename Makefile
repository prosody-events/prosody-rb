# Prosody Ruby Development Makefile

.PHONY: help compile compile-dev test test-tracing clean

help: ## Show this help message
	@echo "Available commands:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

compile: ## Compile Rust extension (release mode)
	bundle exec rake compile

compile-dev: ## Compile Rust extension (development mode)
	bundle exec rake compile:dev

test: ## Run all tests
	PROSODY_SUBSCRIBED_TOPICS=test-topic PROSODY_CASSANDRA_NODES=localhost:9042 bundle exec rspec --format documentation

test-tracing: compile-dev ## Run tracing integration test with OpenTelemetry collector
	OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 \
	OTEL_SERVICE_NAME=prosody-ruby-tracing-test \
	PROSODY_SUBSCRIBED_TOPICS=test-topic \
	PROSODY_CASSANDRA_NODES=localhost:9042 \
	bundle exec rspec --tag tracing --format documentation

clean: ## Clean build artifacts
	bundle exec rake clean