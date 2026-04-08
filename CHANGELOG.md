# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### BUG FIXES

- :bug: email mistake, vici deps ([`e1e5a3f`](https://github.com/estuardodardon/bifrost/commit/e1e5a3fd40b22eb12bbc8120ea7d13cb71438bba))
- :bug: openapi creation ([`9e5bc1a`](https://github.com/estuardodardon/bifrost/commit/9e5bc1a6f9db84cbde8e526aa1faf10b49c71ecd))
- :bug: fix loading model schema ([`7b97c3b`](https://github.com/estuardodardon/bifrost/commit/7b97c3bceee3cd75fcf75fd5c0d6d5973abca055))
- :bug: fix socket connection ([`13e1fd5`](https://github.com/estuardodardon/bifrost/commit/13e1fd5eff205ebf14b94b2d33e7fb006791982b))
- :bug: add libcharon-extra-plugins package depends ([`8d1ca7e`](https://github.com/estuardodardon/bifrost/commit/8d1ca7e63f7e77fa2edd89a5b222018f03dbfd6c))
- :bug: fix Cargo.toml typo ([`0e8a2df`](https://github.com/estuardodardon/bifrost/commit/0e8a2dfcb35f59ad50f8516c78be301ae4347728))
- :bug: fix build_packages.sh typo ([`5045f42`](https://github.com/estuardodardon/bifrost/commit/5045f425636b663183075cc29dbc6f2b9744ab9c))
- :bug: fix New version extract ([`84dcf5f`](https://github.com/estuardodardon/bifrost/commit/84dcf5f0d39f04a09361d5e5f1445e7d91bfdc1f))
- Use writable sqlite path under systemd hardening ([`d424777`](https://github.com/estuardodardon/bifrost/commit/d4247771fc97b3f6b0b1344999857ea3cdccd031))
- :bug: start ike childs on /api/peeers/{peer_name/up ([`43de9fd`](https://github.com/estuardodardon/bifrost/commit/43de9fd3cb09d196f71f74e10769f8413290f567))
- Parsear CHILD_SA en formato moderno de swanctl ([`dc493f8`](https://github.com/estuardodardon/bifrost/commit/dc493f84f43f6b01c81a06b90dc16a4120c21aa2))
- :bug: replace bifrost.service with bifrost-gate.service in build_packages.sh script ([`5821c0f`](https://github.com/estuardodardon/bifrost/commit/5821c0f792ea708a1a9b21513ded2eb3e943b148))
- :bug: remove /heartbeat reference with absulute path ([`cc04f01`](https://github.com/estuardodardon/bifrost/commit/cc04f01a9f201177b42c7af447fd323b31d823c7))

### DOCUMENTATION

- :memo: update changlog file ([`42c1010`](https://github.com/estuardodardon/bifrost/commit/42c1010d1478dc7cd4d9af6181bc639870e8bf51))
- :memo: update changlog file ([`6daf366`](https://github.com/estuardodardon/bifrost/commit/6daf366f8f3024fd67560602923ccaaafce5190c))
- :memo: update changlog file and Cargo.toml app version ([`7a61186`](https://github.com/estuardodardon/bifrost/commit/7a611863a6eb0ef84b6ede3ce2098e7281685163))
- :memo: disable changlog update on build_packages script ([`23c2342`](https://github.com/estuardodardon/bifrost/commit/23c2342e134fde4ad55d0fffe44f99018e2c7967))
- :memo: disable Cargo version update on build_packages script ([`2c0e83e`](https://github.com/estuardodardon/bifrost/commit/2c0e83e96a3dc8c7cc344cb09cb2051401dc81e7))

### FEATURES

- :tada: Init Bifrost proyect ([`bf24c3b`](https://github.com/estuardodardon/bifrost/commit/bf24c3bdeb8c62846e14ba85523dd47b8d514c0d))
- :page_facing_up: rename license file ([`bd2970d`](https://github.com/estuardodardon/bifrost/commit/bd2970d16c22197269b023bf0a3429c887b6286d))
- :tada: add build scripts for deb and rpm packages ([`5bc3ddf`](https://github.com/estuardodardon/bifrost/commit/5bc3ddf60a94de96a88181930d9b9d6e851fb39e))
- :zap: add prometheus support ([`9d0e83e`](https://github.com/estuardodardon/bifrost/commit/9d0e83e5907db6eb72433011547e5f9cfdf965a7))
- :fire: add changlog generator. Include increment_version package ([`21a7a05`](https://github.com/estuardodardon/bifrost/commit/21a7a0564cf81e8a0d8601f3149ee50ae5132641))
- :package: generate version 0.1.0-9 ([`870340f`](https://github.com/estuardodardon/bifrost/commit/870340f53c7b1f563fad75bd68ba71bcf7822c98))
- :package: add strongswan, strongswan-pki, strongswan-swanctl libcharon-extra-plugins package depends ([`144ff07`](https://github.com/estuardodardon/bifrost/commit/144ff0734acaaf9b97692044c94792bab364879b))
- :fire: add loggerhandler ([`fbdeab7`](https://github.com/estuardodardon/bifrost/commit/fbdeab7f091372291564d51af2d6e189d7abb984))
- :fire: add loggerhandler ([`b5c8a59`](https://github.com/estuardodardon/bifrost/commit/b5c8a59083363b88f99d61590bd80b4692ccd25c))
- Integrate real StrongSwan topology parsing ([`8e711f5`](https://github.com/estuardodardon/bifrost/commit/8e711f54c2cca147dd12c4d1b6f89dd9606ff728))
- Move API key management to bifrostctl with DB-backed keys ([`a2b9e92`](https://github.com/estuardodardon/bifrost/commit/a2b9e92d76a0ec398c1d5afed7d74cba18a57efe))
- Add service/connection controls and protect docs with DB users ([`6fee243`](https://github.com/estuardodardon/bifrost/commit/6fee24304188361114612a624f4a842fea2fe104))
- :fire: set root access to bifrostctl ([`1939770`](https://github.com/estuardodardon/bifrost/commit/1939770d74713c4dd95075d7d4ebcbfdb50f3fda))
- :memo: update readme.md info ([`e0a2b90`](https://github.com/estuardodardon/bifrost/commit/e0a2b90a69e48fe5e88ee6c97ee00579a0a6632e))
- :sparkles: add peers status endpoints ([`7ea6808`](https://github.com/estuardodardon/bifrost/commit/7ea68084dcb70abe25569a4cdbb310c8e551e8ac))
- Soportar control por fase y ejemplos en Postman ([`1b8a61e`](https://github.com/estuardodardon/bifrost/commit/1b8a61e8fffced6c5deffef690c27ea9b69dbc13))
- I18n en DB, permisos docs, UI manager y PDF profesional ([`24e7bc1`](https://github.com/estuardodardon/bifrost/commit/24e7bc1d86ecca250f45b83ca3e2720d8fd93443))
- Add heartbeat endpoint with severity model and update Postman auth flow ([`4cbe2cf`](https://github.com/estuardodardon/bifrost/commit/4cbe2cf71159fe6855589b9d0c65ff44caa0af5c))
- Expose endpoint publicly and align payload/contracts ([`4e89d7d`](https://github.com/estuardodardon/bifrost/commit/4e89d7d52831288b07d7039ae9638537a81ab887))
- :fire: chante default port number ([`dfce6c6`](https://github.com/estuardodardon/bifrost/commit/dfce6c676bce57ab8e4339fc9acfbeebed9e433b))
- Ampliar config de conexiones y ejemplos OpenAPI ([`3ea9e05`](https://github.com/estuardodardon/bifrost/commit/3ea9e0521eee7f3023115c9301c20d635c898ce4))

### MISCELLANEOUS

- Update systemd and debian install files ([`027e233`](https://github.com/estuardodardon/bifrost/commit/027e233f8682b3a7a18850709b4acb568da9bb68))
- Anonimizar fixtures y variables de ejemplo ([`1e3c0b0`](https://github.com/estuardodardon/bifrost/commit/1e3c0b0c5c4c56d9453296e9294eef448e315dee))

### REFACTORING

- Remove auth key creation endpoint from service ([`d9effb5`](https://github.com/estuardodardon/bifrost/commit/d9effb55e951094027a26fd478662b88e8f0b740))
- Extraer capa API a módulo dedicado ([`9395541`](https://github.com/estuardodardon/bifrost/commit/939554124423e34c1169fc22b23c883a67e6e7aa))

### BUILD

- Include bifrostctl in deb and rpm scripts ([`dd3e846`](https://github.com/estuardodardon/bifrost/commit/dd3e846c8a273f85223abedc56cad0c184c0cf31))

### SECURITY

- Endurecer arranque, comandos y gestión de swanctl ([`3524cc7`](https://github.com/estuardodardon/bifrost/commit/3524cc7772b9d8e614b77c03607dcac69da37202))

<!-- generated by git-cliff -->
