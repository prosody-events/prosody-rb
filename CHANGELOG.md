# Changelog

## [2.0.0](https://github.com/cincpro/prosody-rb/compare/v1.1.4...v2.0.0) (2025-12-16)


### ⚠ BREAKING CHANGES

* add QoS middleware and rename shutdown to cancel ([#38](https://github.com/cincpro/prosody-rb/issues/38))

### Features

* add QoS middleware and rename shutdown to cancel ([#38](https://github.com/cincpro/prosody-rb/issues/38)) ([622df55](https://github.com/cincpro/prosody-rb/commit/622df5502a7785b218eb341750527267bc05409b))

## [1.1.4](https://github.com/cincpro/prosody-rb/compare/v1.1.3...v1.1.4) (2025-10-28)


### Bug Fixes

* lazily load all module state when it’s used ([#36](https://github.com/cincpro/prosody-rb/issues/36)) ([cb1d396](https://github.com/cincpro/prosody-rb/commit/cb1d3967ab77e2a89b020f51ee18e168abc091b4))

## [1.1.3](https://github.com/cincpro/prosody-rb/compare/v1.1.2...v1.1.3) (2025-09-24)


### Bug Fixes

* eagerly return semaphore permits ([#34](https://github.com/cincpro/prosody-rb/issues/34)) ([fa99a01](https://github.com/cincpro/prosody-rb/commit/fa99a013f4d91f14989c02c6075a95d337651484))

## [1.1.2](https://github.com/cincpro/prosody-rb/compare/v1.1.1...v1.1.2) (2025-09-23)


### Bug Fixes

* only enter runtime if there is no current handle ([#32](https://github.com/cincpro/prosody-rb/issues/32)) ([4f88a0d](https://github.com/cincpro/prosody-rb/commit/4f88a0dd007b203d7ce101c4f4b0bc0ecc33dc0c))

## [1.1.1](https://github.com/cincpro/prosody-rb/compare/v1.1.0...v1.1.1) (2025-09-19)


### Bug Fixes

* remove unsafe usages of frozen sharable and fix tests ([#30](https://github.com/cincpro/prosody-rb/issues/30)) ([84ed9b1](https://github.com/cincpro/prosody-rb/commit/84ed9b16437bcc4ae688acb0a4e77bc620d7a9c7))

## [1.1.0](https://github.com/cincpro/prosody-rb/compare/v1.0.6...v1.1.0) (2025-09-19)


### Features

* expose source system ([#27](https://github.com/cincpro/prosody-rb/issues/27)) ([e73f229](https://github.com/cincpro/prosody-rb/commit/e73f2297cc9941d0674cbce85e0dfc7a93ce43f0))


### Bug Fixes

* propagate trace context through timer methods ([#29](https://github.com/cincpro/prosody-rb/issues/29)) ([48f0b83](https://github.com/cincpro/prosody-rb/commit/48f0b838c8674c59b43eb4d75a58c1743873e198))

## [1.0.6](https://github.com/cincpro/prosody-rb/compare/v1.0.5...v1.0.6) (2025-09-12)


### Performance Improvements

* bound message buffering ([#25](https://github.com/cincpro/prosody-rb/issues/25)) ([3acf90b](https://github.com/cincpro/prosody-rb/commit/3acf90b15644fb180c4e6092ef9653db56673ea7))

## [1.0.5](https://github.com/cincpro/prosody-rb/compare/v1.0.4...v1.0.5) (2025-09-04)


### Bug Fixes

* prevent premature AsyncTaskProcessor shutdown during partition rebalancing ([#22](https://github.com/cincpro/prosody-rb/issues/22)) ([170b557](https://github.com/cincpro/prosody-rb/commit/170b5572cd9fa58523adc2ef854f2928ddaf3523))

## [1.0.4](https://github.com/cincpro/prosody-rb/compare/v1.0.3...v1.0.4) (2025-08-22)


### Bug Fixes

* add migration locks and jitter to timer slab loads ([9377fe9](https://github.com/cincpro/prosody-rb/commit/9377fe914a7153392f5443114b685b50121cdf88))

## [1.0.3](https://github.com/cincpro/prosody-rb/compare/v1.0.2...v1.0.3) (2025-08-12)


### Bug Fixes

* update rb_sys ([#19](https://github.com/cincpro/prosody-rb/issues/19)) ([d1a7973](https://github.com/cincpro/prosody-rb/commit/d1a79732cbfb28f3efa283c34355f7cd7d310212))

## [1.0.2](https://github.com/cincpro/prosody-rb/compare/v1.0.1...v1.0.2) (2025-08-12)


### Bug Fixes

* bump prosody to pick up OTEL updates ([#17](https://github.com/cincpro/prosody-rb/issues/17)) ([65046c6](https://github.com/cincpro/prosody-rb/commit/65046c694616ac248863c34bb9363cb580196204))

## [1.0.1](https://github.com/cincpro/prosody-rb/compare/v1.0.0...v1.0.1) (2025-07-23)


### Bug Fixes

* don’t execute timer commands while shutting down the partition ([#15](https://github.com/cincpro/prosody-rb/issues/15)) ([474fa24](https://github.com/cincpro/prosody-rb/commit/474fa2406bb253e2a5a488012ba6c89914603425))

## [1.0.0](https://github.com/cincpro/prosody-rb/compare/v0.3.0...v1.0.0) (2025-07-16)


### ⚠ BREAKING CHANGES

* add timer scheduling support with Cassandra persistence ([#13](https://github.com/cincpro/prosody-rb/issues/13))

### Features

* add timer scheduling support with Cassandra persistence ([#13](https://github.com/cincpro/prosody-rb/issues/13)) ([bedcaba](https://github.com/cincpro/prosody-rb/commit/bedcaba5ddd055097c345a8f2ba2e5f96b74808d))

## [0.3.0](https://github.com/cincpro/prosody-rb/compare/v0.2.1...v0.3.0) (2025-06-11)


### Features

* support Configuration objects in Client.new and make probe_port optional ([#11](https://github.com/cincpro/prosody-rb/issues/11)) ([0e472d1](https://github.com/cincpro/prosody-rb/commit/0e472d169b1d20bffd9f54320bf8421796a7fa45))

## [0.2.1](https://github.com/cincpro/prosody-rb/compare/v0.2.0...v0.2.1) (2025-05-05)


### Bug Fixes

* ensure all cancellation tasks are cleaned up ([#10](https://github.com/cincpro/prosody-rb/issues/10)) ([5e16f3f](https://github.com/cincpro/prosody-rb/commit/5e16f3fb19efa96200b56f7c685f3a71729bd0be))
* type signatures ([#7](https://github.com/cincpro/prosody-rb/issues/7)) ([4140d9d](https://github.com/cincpro/prosody-rb/commit/4140d9d7918b8c0461e31e70efbbb1040eb84fe5))

## 0.2.0 (2025-05-02)


### Features

* async task scheduling ([96ea83a](https://github.com/cincpro/prosody-rb/commit/96ea83a4b53b14506bb608e3e5e15faa4f59191f))
* basic async bridge ([26ce6a6](https://github.com/cincpro/prosody-rb/commit/26ce6a66f08d7195d323eea88d07fc6c54f6df85))
* bidirectional bridge working ([b7322f9](https://github.com/cincpro/prosody-rb/commit/b7322f9e7d31054e8a94ed4fdc4f2beb3a76e8e8))
* configuration object ([f283c95](https://github.com/cincpro/prosody-rb/commit/f283c952137da66787f32f0e7d67aa48c37304c5))
* map message and context ([611ebcf](https://github.com/cincpro/prosody-rb/commit/611ebcf6890e95596e50f5ae65a5a494716eae86))
* otel and logging support ([#3](https://github.com/cincpro/prosody-rb/issues/3)) ([95908f6](https://github.com/cincpro/prosody-rb/commit/95908f6a335fc53d2e86159b6bbfc904c02814e1))
* permanent and transient exception support ([#4](https://github.com/cincpro/prosody-rb/issues/4)) ([29a947b](https://github.com/cincpro/prosody-rb/commit/29a947b634736acef49d6390725154d73172a864))
* scheduler lifecycle support ([bccb82f](https://github.com/cincpro/prosody-rb/commit/bccb82f98b500d10e33940c9dacf5b76b77c95fc))
* support production ([67fedd2](https://github.com/cincpro/prosody-rb/commit/67fedd27d0c84960bfc9d7a5614a523ce18badf8))
* wire up subscribe ([633a3a2](https://github.com/cincpro/prosody-rb/commit/633a3a2f475d304995e1bb8872da0978d455b605))


### Bug Fixes

* protect queues from garbage collection ([6b970f1](https://github.com/cincpro/prosody-rb/commit/6b970f11b25a7fb3c028cf52c7ee27073016673e))
* remove exception type checking ([85c524d](https://github.com/cincpro/prosody-rb/commit/85c524d6b24fd713f91bc1a3e2a4c0129622b271))
* subscription specs and probe port deserialization ([d636db0](https://github.com/cincpro/prosody-rb/commit/d636db0a2b633e1f1b0410c8fc129ad2b2f402dd))
* task cancellation ([49b79e0](https://github.com/cincpro/prosody-rb/commit/49b79e09ac9271a6baadbf7669d02550d285b12e))


### Performance Improvements

* release the gvl less often, since it’s expensive ([e4aecdf](https://github.com/cincpro/prosody-rb/commit/e4aecdfd56cb57e0f9ffc611630724d6a5a03e66))


### Miscellaneous Chores

* release 0.2.0 ([55cf67e](https://github.com/cincpro/prosody-rb/commit/55cf67e74cf7dc846c105e1230ec9aceee85c7f7))
