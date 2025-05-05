# Changelog

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
