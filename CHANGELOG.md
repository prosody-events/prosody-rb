# Changelog

## 0.1.0 (2026-04-14)


### ⚠ BREAKING CHANGES

* add QoS middleware and rename shutdown to cancel ([#38](https://github.com/prosody-events/prosody-rb/issues/38))
* add timer scheduling support with Cassandra persistence ([#13](https://github.com/prosody-events/prosody-rb/issues/13))

### Features

* add QoS middleware and rename shutdown to cancel ([#38](https://github.com/prosody-events/prosody-rb/issues/38)) ([1f5c476](https://github.com/prosody-events/prosody-rb/commit/1f5c476978307dc2dcf1336f72e5c7dd198a1e53))
* add timer scheduling support with Cassandra persistence ([#13](https://github.com/prosody-events/prosody-rb/issues/13)) ([acb112c](https://github.com/prosody-events/prosody-rb/commit/acb112ca89edd011ca50f181c1c1bf42dcc7024c))
* async task scheduling ([96ea83a](https://github.com/prosody-events/prosody-rb/commit/96ea83a4b53b14506bb608e3e5e15faa4f59191f))
* basic async bridge ([26ce6a6](https://github.com/prosody-events/prosody-rb/commit/26ce6a66f08d7195d323eea88d07fc6c54f6df85))
* bidirectional bridge working ([b7322f9](https://github.com/prosody-events/prosody-rb/commit/b7322f9e7d31054e8a94ed4fdc4f2beb3a76e8e8))
* configuration error surfacing, per-type timer semaphores, timer read performance ([#71](https://github.com/prosody-events/prosody-rb/issues/71)) ([3710cb0](https://github.com/prosody-events/prosody-rb/commit/3710cb0a1c6e2e484abc6ba90e1b34a42b4ed2a0))
* configuration object ([f283c95](https://github.com/prosody-events/prosody-rb/commit/f283c952137da66787f32f0e7d67aa48c37304c5))
* expose source system ([#27](https://github.com/prosody-events/prosody-rb/issues/27)) ([56b1b3c](https://github.com/prosody-events/prosody-rb/commit/56b1b3c22c8af36152f9d0be3da0748588a682e0))
* expose telemetry emitter configuration to Ruby client ([#64](https://github.com/prosody-events/prosody-rb/issues/64)) ([436cc1e](https://github.com/prosody-events/prosody-rb/commit/436cc1e985ebfa442f91af020fc6027112fd3575))
* map message and context ([611ebcf](https://github.com/prosody-events/prosody-rb/commit/611ebcf6890e95596e50f5ae65a5a494716eae86))
* non-blocking timer retry ([#50](https://github.com/prosody-events/prosody-rb/issues/50)) ([02bb23a](https://github.com/prosody-events/prosody-rb/commit/02bb23a48d4ff5993b0e2e29fa335b51dd3caef6))
* otel and logging support ([#3](https://github.com/prosody-events/prosody-rb/issues/3)) ([0b8ddf8](https://github.com/prosody-events/prosody-rb/commit/0b8ddf8012d035d6f65f72fa22f3cca2766bd5f0))
* permanent and transient exception support ([#4](https://github.com/prosody-events/prosody-rb/issues/4)) ([df10f92](https://github.com/prosody-events/prosody-rb/commit/df10f9297671f257f4873ce91cfecae2c1f28a41))
* persistent deduplication with global cache and Cassandra backend ([#75](https://github.com/prosody-events/prosody-rb/issues/75)) ([a1adf16](https://github.com/prosody-events/prosody-rb/commit/a1adf16ddb88b7312b221d894b36dd4301211307))
* protect client methods against post-fork usage ([#78](https://github.com/prosody-events/prosody-rb/issues/78)) ([2aa2f4f](https://github.com/prosody-events/prosody-rb/commit/2aa2f4f221a9b454a12fd0ebc74c1b43352cc11b))
* scheduler lifecycle support ([bccb82f](https://github.com/prosody-events/prosody-rb/commit/bccb82f98b500d10e33940c9dacf5b76b77c95fc))
* Sentry error monitoring for handler dispatch ([#76](https://github.com/prosody-events/prosody-rb/issues/76)) ([aa56ad1](https://github.com/prosody-events/prosody-rb/commit/aa56ad10f011030d312229999765bdbedb9c2ed4))
* shutdown grace period and configurable span relation ([#79](https://github.com/prosody-events/prosody-rb/issues/79)) ([7a61661](https://github.com/prosody-events/prosody-rb/commit/7a61661885a325cab3e76a2382371e7195fba818))
* support Configuration objects in Client.new and make probe_port optional ([#11](https://github.com/prosody-events/prosody-rb/issues/11)) ([6efe4a3](https://github.com/prosody-events/prosody-rb/commit/6efe4a361c5bc90654e8a6374c44ca2946830858))
* support production ([67fedd2](https://github.com/prosody-events/prosody-rb/commit/67fedd27d0c84960bfc9d7a5614a523ce18badf8))
* warn when SENTRY_DSN is set but sentry-ruby is not installed ([#77](https://github.com/prosody-events/prosody-rb/issues/77)) ([214b70b](https://github.com/prosody-events/prosody-rb/commit/214b70bd51264b204354ebc6eed34a47284ebae1))
* wire up subscribe ([633a3a2](https://github.com/prosody-events/prosody-rb/commit/633a3a2f475d304995e1bb8872da0978d455b605))


### Bug Fixes

* add migration locks and jitter to timer slab loads ([4cdc02f](https://github.com/prosody-events/prosody-rb/commit/4cdc02f8bfa5775ead2b0f1c9e48a6c8e6fb3814))
* auto-initialize Sentry from SENTRY_DSN when sentry-ruby is installed ([#84](https://github.com/prosody-events/prosody-rb/issues/84)) ([0fb5a28](https://github.com/prosody-events/prosody-rb/commit/0fb5a28ab939a3c41b4c6f1bcee7e52738a70546))
* bump prosody to pick up OTEL updates ([#17](https://github.com/prosody-events/prosody-rb/issues/17)) ([0bf39a6](https://github.com/prosody-events/prosody-rb/commit/0bf39a6c4bbf504d85af0d0a05bef0d5a5341e75))
* cache parent context instead of span in Kafka loader ([#44](https://github.com/prosody-events/prosody-rb/issues/44)) ([7299c19](https://github.com/prosody-events/prosody-rb/commit/7299c1960ef908043cc793ed73ef5f89d022eab0))
* don’t execute timer commands while shutting down the partition ([#15](https://github.com/prosody-events/prosody-rb/issues/15)) ([d077736](https://github.com/prosody-events/prosody-rb/commit/d077736a3b04920bcc7cda37cafe57c7a966c958))
* downgrade span parent extraction failures to debug level ([#42](https://github.com/prosody-events/prosody-rb/issues/42)) ([d345af9](https://github.com/prosody-events/prosody-rb/commit/d345af925880152faf59ff4b7784cd5cc2cbdd4e))
* eagerly return semaphore permits ([#34](https://github.com/prosody-events/prosody-rb/issues/34)) ([58d0f85](https://github.com/prosody-events/prosody-rb/commit/58d0f855880e4da00bd22b9765976239335396f1))
* ensure all cancellation tasks are cleaned up ([#10](https://github.com/prosody-events/prosody-rb/issues/10)) ([2024ab9](https://github.com/prosody-events/prosody-rb/commit/2024ab95b6b785800b2ff1726dbbe0efb1249a0d))
* expose configurable logger via Prosody.logger ([#73](https://github.com/prosody-events/prosody-rb/issues/73)) ([a3cc51e](https://github.com/prosody-events/prosody-rb/commit/a3cc51e3f8b389e8928675ad32426636bdcefcd8))
* graceful slab_loader shutdown, remove timer backpressure gaps, and new API surface ([#55](https://github.com/prosody-events/prosody-rb/issues/55)) ([1c0ac1c](https://github.com/prosody-events/prosody-rb/commit/1c0ac1c0b5d9dd46f8ab70042afb8fba6fc0bd92))
* install sentry-ruby gem in CI test jobs ([#82](https://github.com/prosody-events/prosody-rb/issues/82)) ([4042838](https://github.com/prosody-events/prosody-rb/commit/404283813be9d4f016dbfa50ea4b8f429db5fe1f))
* lazily load all module state when it’s used ([#36](https://github.com/prosody-events/prosody-rb/issues/36)) ([3d94825](https://github.com/prosody-events/prosody-rb/commit/3d9482558798bf86bde3d14d4c1ef32df105ad7a))
* only enter runtime if there is no current handle ([#32](https://github.com/prosody-events/prosody-rb/issues/32)) ([1c94a05](https://github.com/prosody-events/prosody-rb/commit/1c94a05696cc3eb511c71aa7887edb55bec49d04))
* only log events, not spans ([#54](https://github.com/prosody-events/prosody-rb/issues/54)) ([184f43b](https://github.com/prosody-events/prosody-rb/commit/184f43b9b5290d4703631cef5b78cb4720236688))
* prevent false OffsetDeleted errors from concurrent loader requests ([#62](https://github.com/prosody-events/prosody-rb/issues/62)) ([ddfa4e4](https://github.com/prosody-events/prosody-rb/commit/ddfa4e4333874ea53fe1e36fce72fecc4a61dae1))
* prevent premature AsyncTaskProcessor shutdown during partition rebalancing ([#22](https://github.com/prosody-events/prosody-rb/issues/22)) ([f2e62f5](https://github.com/prosody-events/prosody-rb/commit/f2e62f5da470ceb5ed9880ab3a108de2e3bb9f66))
* prevent segfault when ThreadSafeValue is dropped on a non-Ruby thread ([#80](https://github.com/prosody-events/prosody-rb/issues/80)) ([07d4c3a](https://github.com/prosody-events/prosody-rb/commit/07d4c3aa21baa7a05f2ef8c8a703af242124dcc0))
* propagate trace context through timer methods ([#29](https://github.com/prosody-events/prosody-rb/issues/29)) ([4b2ce12](https://github.com/prosody-events/prosody-rb/commit/4b2ce12e3d028e64889acd4cc7e522edeaa38e9f))
* properly cancel worker tasks using Async::Stop ([#40](https://github.com/prosody-events/prosody-rb/issues/40)) ([cdac4dc](https://github.com/prosody-events/prosody-rb/commit/cdac4dc302efaf902fb135b71e5c45de9b28d911))
* properly clean up Ruby values dropped on non-Ruby threads ([#81](https://github.com/prosody-events/prosody-rb/issues/81)) ([8ad55cc](https://github.com/prosody-events/prosody-rb/commit/8ad55ccb0b737ce0ce6f3c550d7a6d6bef3d3106))
* protect queues from garbage collection ([6b970f1](https://github.com/prosody-events/prosody-rb/commit/6b970f11b25a7fb3c028cf52c7ee27073016673e))
* **release:** configure release-please to update version.rb ([#5](https://github.com/prosody-events/prosody-rb/issues/5)) ([f51d5d3](https://github.com/prosody-events/prosody-rb/commit/f51d5d32e070bf486eb8b2ffccedbc7c6171ba7a))
* remove exception type checking ([85c524d](https://github.com/prosody-events/prosody-rb/commit/85c524d6b24fd713f91bc1a3e2a4c0129622b271))
* remove max_enqueued_per_key from type signatures ([#56](https://github.com/prosody-events/prosody-rb/issues/56)) ([251f47d](https://github.com/prosody-events/prosody-rb/commit/251f47dbb078e4866e7552eab55bdda15efcfdad))
* remove unsafe usages of frozen sharable and fix tests ([#30](https://github.com/prosody-events/prosody-rb/issues/30)) ([6db7b03](https://github.com/prosody-events/prosody-rb/commit/6db7b036730fc8cb421fdb150ca9e680d66e6d9b))
* restore OTel context in async fibers and defer span finish ([#60](https://github.com/prosody-events/prosody-rb/issues/60)) ([f545a40](https://github.com/prosody-events/prosody-rb/commit/f545a40f426b4a55ef9bfb7ae23760014ce595e7))
* subscription specs and probe port deserialization ([d636db0](https://github.com/prosody-events/prosody-rb/commit/d636db0a2b633e1f1b0410c8fc129ad2b2f402dd))
* task cancellation ([49b79e0](https://github.com/prosody-events/prosody-rb/commit/49b79e09ac9271a6baadbf7669d02550d285b12e))
* telemetry event_time uses millisecond precision with Z suffix ([#69](https://github.com/prosody-events/prosody-rb/issues/69)) ([1ca1a2e](https://github.com/prosody-events/prosody-rb/commit/1ca1a2ec3b03218ce96fae01021a606f9edfa4b0))
* **test:** increase MESSAGE_TIMEOUT to 30s for CI reliability ([#7](https://github.com/prosody-events/prosody-rb/issues/7)) ([4d66077](https://github.com/prosody-events/prosody-rb/commit/4d6607737a2b12665f7c6ae175e60fdf7cd3dff6))
* type signatures ([#7](https://github.com/prosody-events/prosody-rb/issues/7)) ([94f6a48](https://github.com/prosody-events/prosody-rb/commit/94f6a48c8850a1a886850f926a654dba89c0ac1e))
* update prosody to fix partition resume after rebalance ([#48](https://github.com/prosody-events/prosody-rb/issues/48)) ([3b62cf9](https://github.com/prosody-events/prosody-rb/commit/3b62cf90a172ba9b686e4b651aa52527f4fb4eb3))
* update prosody to fix timeout-induced partition stalls ([#46](https://github.com/prosody-events/prosody-rb/issues/46)) ([3b55bb8](https://github.com/prosody-events/prosody-rb/commit/3b55bb81140bed050b031491f1771390dc6d3d45))
* update rb_sys ([#19](https://github.com/prosody-events/prosody-rb/issues/19)) ([90898b3](https://github.com/prosody-events/prosody-rb/commit/90898b3db496c55d6be6f5df0b765a96e7216fa6))


### Performance Improvements

* bound message buffering ([#25](https://github.com/prosody-events/prosody-rb/issues/25)) ([e2d42ab](https://github.com/prosody-events/prosody-rb/commit/e2d42ab6a27f10724ce45b44c4b8388b23577a04))
* release the gvl less often, since it’s expensive ([e4aecdf](https://github.com/prosody-events/prosody-rb/commit/e4aecdfd56cb57e0f9ffc611630724d6a5a03e66))
* use jemalloc allocator ([#52](https://github.com/prosody-events/prosody-rb/issues/52)) ([73bfd8a](https://github.com/prosody-events/prosody-rb/commit/73bfd8afd62c6d41f8b45b3ff76263476758b9e0))


### Miscellaneous Chores

* release 0.1.0 ([a077e3c](https://github.com/prosody-events/prosody-rb/commit/a077e3caa503023cc3ddde03cfb642a3fb966821))
* release 0.2.0 ([911a476](https://github.com/prosody-events/prosody-rb/commit/911a476a04d3c151ab3a1a6a86c6419e8984d486))
