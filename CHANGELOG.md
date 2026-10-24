# Changelog

## [0.3.1](https://github.com/getcihub/cihub/compare/v0.3.0...v0.3.1) (2026-10-24)


### Bug Fixes

* **platform:** keep a membership when GitHub refuses the role read or the account was renamed ([d8722ee](https://github.com/getcihub/cihub/commit/d8722ee49015b33cd0ad244c926ce704e845a14d))
* **platform:** read the repositories at once after a reinstall on the same account ([b33e6e4](https://github.com/getcihub/cihub/commit/b33e6e460ed26ae1ccaa7c21f6bdd0a57123ba4d))
* **platform:** reconnect the organization when the App is installed again on the same account ([e868021](https://github.com/getcihub/cihub/commit/e86802145cc104a29da09451054a17b8eda71500))
* **web:** authorize a session from the decoded path parameters ([56afe88](https://github.com/getcihub/cihub/commit/56afe88554e83810b5f8e104d272bf0fb610dc32))

## [0.3.0](https://github.com/getcihub/cihub/compare/v0.2.1...v0.3.0) (2026-10-24)


### ⚠ BREAKING CHANGES

* **cli:** sign the command line in with a device code ([#116](https://github.com/getcihub/cihub/issues/116))
* split the cihub command line and the cihub-server binary ([#114](https://github.com/getcihub/cihub/issues/114))

### Features

* a viewer for the visual diff of the dashboard ([3ab3f8f](https://github.com/getcihub/cihub/commit/3ab3f8f7984eb1b00165a081ccab9f0bf1a1b437))
* an observability profile in compose.yaml ([95b2b0d](https://github.com/getcihub/cihub/commit/95b2b0dc78dd9b305dfcd239ea1cd6021a27ca33))
* **analytics:** the Health of a period that the person chooses ([7d7046c](https://github.com/getcihub/cihub/commit/7d7046c748fadde75714ddf9cf0f769e8610c12a))
* **analytics:** the Health of a queue, from facts derived from its log ([92b8ff2](https://github.com/getcihub/cihub/commit/92b8ff2984f5891175c759ab2f2d025b5f876118))
* **api:** repository setup interfaces and repository routes ([#144](https://github.com/getcihub/cihub/issues/144)) ([e68f0ba](https://github.com/getcihub/cihub/commit/e68f0ba16efecd86df1b3eb85304550017dd798c))
* **cli:** sign the command line in with a device code ([#116](https://github.com/getcihub/cihub/issues/116)) ([5355ce8](https://github.com/getcihub/cihub/commit/5355ce8f5cc66c742756ac4ee0d5e48a81019f67))
* **config:** conditions as typed YAML maps, with a JSON schema ([#157](https://github.com/getcihub/cihub/issues/157)) ([c3055aa](https://github.com/getcihub/cihub/commit/c3055aa55e584d7f5fa37f9512324428bffd62d2))
* icons on the buttons of the queue ([#100](https://github.com/getcihub/cihub/issues/100)) ([b64f130](https://github.com/getcihub/cihub/commit/b64f1302041d3ae75715f09aebb0870dd72e1b73))
* **mergequeue:** a PR shows whether it can enter its queue ([3526c8d](https://github.com/getcihub/cihub/commit/3526c8d913cd3e5b3f9b1f390eea9df413ee0fa3))
* **mergequeue:** direct merge is on by default ([3a2ac8f](https://github.com/getcihub/cihub/commit/3a2ac8f8eae7453227665056b4561c0f9f22ec77))
* **mergequeue:** one queue per target branch ([#143](https://github.com/getcihub/cihub/issues/143)) ([9ee3c11](https://github.com/getcihub/cihub/commit/9ee3c1151e70efef3bb950529e5bb913892b4209))
* **mergequeue:** read and show the setup of a repository ([#149](https://github.com/getcihub/cihub/issues/149)) ([c77bf29](https://github.com/getcihub/cihub/commit/c77bf2968b40bedb8823e05e5917ca6d187ef945))
* **mergequeue:** the page of a PR, its draft PR and its checks in the drawer ([289358d](https://github.com/getcihub/cihub/commit/289358d24779f80020cdb8308c01d4e2819bfd9c))
* **mergequeue:** the status of an awaited required check is a closed vocabulary ([60b1584](https://github.com/getcihub/cihub/commit/60b1584169de36594b707bc857871f1107a33217))
* **platform:** access per repository with the token of the person ([#147](https://github.com/getcihub/cihub/issues/147)) ([43302a2](https://github.com/getcihub/cihub/commit/43302a290f1cb3a868fbecac7c4b4c649066a5e6))
* **platform:** Setup URL and a sign-in that connects installations ([#146](https://github.com/getcihub/cihub/issues/146)) ([ae4025e](https://github.com/getcihub/cihub/commit/ae4025e9bd21181f24b469925508cb5883fc161d))
* **scm:** read the page of a PR, and the state and times of a check ([ca3fb10](https://github.com/getcihub/cihub/commit/ca3fb1002a9bd77fa1b61da5181036d05d59acfb))
* **server:** database pool settings and encryption key ([#145](https://github.com/getcihub/cihub/issues/145)) ([92cc576](https://github.com/getcihub/cihub/commit/92cc5762ec295cde515ef710b78fffc6e9851a5c))
* **site:** cihub.io, its documentation, and the GitHub App from the command line ([9e57c21](https://github.com/getcihub/cihub/commit/9e57c219f51d5ba8d910e8fdd58fe248930baab3))
* split the cihub command line and the cihub-server binary ([#114](https://github.com/getcihub/cihub/issues/114)) ([96d44cc](https://github.com/getcihub/cihub/commit/96d44cc952e59f960e3e1049abe3a52a35161495))
* the dashboard and its API ([0758ac0](https://github.com/getcihub/cihub/commit/0758ac0be0e25c3ef97dea15c6fca2c10bc736eb))
* the empty state of a queue ([#103](https://github.com/getcihub/cihub/issues/103)) ([735ff14](https://github.com/getcihub/cihub/commit/735ff14e9f06cc4e006e70adf8120565bbb527f5))
* **web:** an avatar-only account menu, with a choice of appearance ([cba2239](https://github.com/getcihub/cihub/commit/cba2239e244fcce81f809c483cba89e2029d3b17))
* **web:** banners with a neutral surface and a rule of their tone ([#153](https://github.com/getcihub/cihub/issues/153)) ([2e952ca](https://github.com/getcihub/cihub/commit/2e952ca644cc3f24ac3d1b525b8b753ade40ffca))
* **web:** onboarding and repository setup pages ([#150](https://github.com/getcihub/cihub/issues/150)) ([4002cd2](https://github.com/getcihub/cihub/commit/4002cd2f9f2443e105feb1461f270263c59c266b))
* **web:** the copy of the command says so for a moment, in a toast ([2da2a4c](https://github.com/getcihub/cihub/commit/2da2a4cc743f002d1074c91842fab62fe48a4098))
* **web:** the empty graph, the tab counters and the history of a PR ([#108](https://github.com/getcihub/cihub/issues/108)) ([eb1eb8a](https://github.com/getcihub/cihub/commit/eb1eb8a333fe5d9b083373d25c40649f3116e860))
* **web:** the empty states before the first queue ([471cd99](https://github.com/getcihub/cihub/commit/471cd995ba501b1a1b58edf53e0d958477b17f05))
* **web:** the live graph says whether it follows the queue ([c9f1548](https://github.com/getcihub/cihub/commit/c9f15488296698b266838a99c57a07d2d400cef2))
* **web:** the menu of the person and the PRs of a queue show the avatar, from the SCM profile ([#120](https://github.com/getcihub/cihub/issues/120)) ([707b0b6](https://github.com/getcihub/cihub/commit/707b0b638172babad695317a060d858f315c9931))
* **web:** the queue actions and the live mark follow the state of the queue ([410e9cc](https://github.com/getcihub/cihub/commit/410e9cc15d8b22d5d7a0dfbe8ab3fb8f7db1a0b0))
* **web:** the repository picker opens the only queue of a repository ([a09e476](https://github.com/getcihub/cihub/commit/a09e476b6e8663e4334613779731caaeec67b351))
* **web:** what blocks a PR in its drawer, its queue conditions as the condition builder ([a10241f](https://github.com/getcihub/cihub/commit/a10241f9a24328ebe180b762355db8d40fb2a9e2))


### Bug Fixes

* bugs and simplifications from a code review ([#152](https://github.com/getcihub/cihub/issues/152)) ([2413a5e](https://github.com/getcihub/cihub/commit/2413a5ea4cf5ac6fc45bebc0d84d288abaab300c))
* **cihub:** update configuration ([2d4beca](https://github.com/getcihub/cihub/commit/2d4becab7fabe4da60602dc9578d19f6efb98ba3))
* **deps:** update dependency zod to v4.6.6 ([ad43291](https://github.com/getcihub/cihub/commit/ad432913390acb4851de32e82df4de9672c42e23))
* **deps:** update rustls to 0.23.46 ([f633791](https://github.com/getcihub/cihub/commit/f63379136c4f6b88f0c7871dae57fd0a830b5c6f))
* **deps:** update rustls-webpki and webpki-root-certs for the emSign advisories ([6b7cc06](https://github.com/getcihub/cihub/commit/6b7cc06dff002de2e8a8dfcf9bf0e88203132f16))
* **deps:** update tanstack-query monorepo to v5.104.2 ([601357e](https://github.com/getcihub/cihub/commit/601357ece7091d5eac7d3a15d9e5ee82e5e825a6))
* **deps:** update tanstack-router monorepo ([#104](https://github.com/getcihub/cihub/issues/104)) ([6d9271d](https://github.com/getcihub/cihub/commit/6d9271d81989313752a024d9f03533b7dc6fb80e))
* **deps:** update tokio to 1.53.3 for RUSTSEC-2026-0386 ([fc747f1](https://github.com/getcihub/cihub/commit/fc747f1c1cc08a63ad1db2e8a5cb30acf96bd29c))
* **mergequeue:** a push of a merge waits while the success of the merge waits in the inbox ([f05d5e4](https://github.com/getcihub/cihub/commit/f05d5e48e8130fc81e34cbbefadd57230cbdbdd2))
* **mergequeue:** welcome a PR opened before its queue at its next change ([9b6973a](https://github.com/getcihub/cihub/commit/9b6973a56405fa6bece98f5d2b31b889ae34318c))
* **platform:** keep the permissions of a person after their sign-out ([06c3a2a](https://github.com/getcihub/cihub/commit/06c3a2a876d62d52fe602862360d2ded64099fa8))
* the visual diff of views whose size changed, and the zoomed split ([#106](https://github.com/getcihub/cihub/issues/106)) ([ad83000](https://github.com/getcihub/cihub/commit/ad830003ba626f192ea468e9d26792c25412baf7))
* **web:** align the sizes and weights of the text with the design ([#107](https://github.com/getcihub/cihub/issues/107)) ([d159efc](https://github.com/getcihub/cihub/commit/d159efc9c1c8354aef22968ad37e8c5b2885a545))
* **web:** follow a change of the system theme while the page is open ([674c39b](https://github.com/getcihub/cihub/commit/674c39b91c6a03478b2fb1bef0af7558fb99d504))
* **web:** ignore anti-aliased edges in the visual comparison ([#156](https://github.com/getcihub/cihub/issues/156)) ([c5c3f5c](https://github.com/getcihub/cihub/commit/c5c3f5c6c3d5c4c39ef627049ba4cb3479bd871c))
* **web:** say that an empty repository has no commit yet ([#155](https://github.com/getcihub/cihub/issues/155)) ([5d859d2](https://github.com/getcihub/cihub/commit/5d859d2130fd1d7d2c957ad8bf356a17b2114b8d))
* **web:** the summary of the period picker wraps instead of widening it ([47beb88](https://github.com/getcihub/cihub/commit/47beb88af9febf5328dd3adc7e46ac4c4d90dae9))
* **web:** the tabs and the panes of a queue keep the scroll position ([40330e9](https://github.com/getcihub/cihub/commit/40330e9e02c8872066fc656da86ba863331dbdec))
* **web:** the visual diff skips a view that the base cannot show ([736ee2c](https://github.com/getcihub/cihub/commit/736ee2c0d11fabc4799dec8fb48f59782af125a0))


### Performance Improvements

* **mergequeue:** read the queues and the PRs through indexes, not their history ([#109](https://github.com/getcihub/cihub/issues/109)) ([43b478f](https://github.com/getcihub/cihub/commit/43b478f3348a52114faa07cdda96076fad4c6632))


### Code Refactoring

* **deploy:** group the chart and the compose installation under deploy/ ([fd834bf](https://github.com/getcihub/cihub/commit/fd834bf5fb5012453f359673d4b10a1d23e38c99))
* simplify the server code and the dashboard layout ([#92](https://github.com/getcihub/cihub/issues/92)) ([53097b6](https://github.com/getcihub/cihub/commit/53097b6a0508391b3aa422b1108156827b48084e))
* the hooks of the design system in components/ui/hooks ([#101](https://github.com/getcihub/cihub/issues/101)) ([1695e60](https://github.com/getcihub/cihub/commit/1695e60b52c55ae393e4805981eabfa150d3a5ac))


### Tests

* **mergequeue:** compare the pages of the PRs without their estimate ([e5855e3](https://github.com/getcihub/cihub/commit/e5855e37ec7422b71387a5e94b9ee54433f93a87))
* **web:** show a scrolling drawer whole in the visual captures ([82c07a3](https://github.com/getcihub/cihub/commit/82c07a38f671fe4b51da4f75f862e340a58def13))
* write the JUnit report of each suite into test-reports/ ([78c4a86](https://github.com/getcihub/cihub/commit/78c4a86b0182d1777a1a92e19a134d7f471cf639))


### Build System

* move the chart to helm/ and make compose.yaml the development stack ([b896336](https://github.com/getcihub/cihub/commit/b8963361be1225eea1e32e52879a8607a54fb8be))
* name every mise task &lt;domain&gt;:&lt;action&gt; ([df707ab](https://github.com/getcihub/cihub/commit/df707ab7ee2efe1f681cd85db30a1ebec7423b23))
* rename the mise tasks, update the smoke test and the docs ([#91](https://github.com/getcihub/cihub/issues/91)) ([79fd7a8](https://github.com/getcihub/cihub/commit/79fd7a8f9879ce7ae08e5cc80cbe653de695e074))


### Continuous Integration

* **renovate:** run only for the pull requests and the dashboard of Renovate ([49d54df](https://github.com/getcihub/cihub/commit/49d54df57a21bdb282240e6ac8ff869376c7da0a))
* run the lint of the dashboard in its own job ([cd6c9ad](https://github.com/getcihub/cihub/commit/cd6c9ad966015bc42b5df609e6c4979b61df9a1e))
* run the unit tests of the dashboard in their own job ([561a57c](https://github.com/getcihub/cihub/commit/561a57c54b7e612f45aacd19bf33ec99620072af))
* **web:** compare the views of the dashboard with the base on each pull request ([#128](https://github.com/getcihub/cihub/issues/128)) ([fc6ef36](https://github.com/getcihub/cihub/commit/fc6ef36605b19d49d9d8b7490241da5d04f053ac))
* **web:** the visual artifact keeps only the views that differ ([#130](https://github.com/getcihub/cihub/issues/130)) ([300d4a1](https://github.com/getcihub/cihub/commit/300d4a1a56f9c89f032afd4d1a89d7ca85ba90a3))


### Miscellaneous Chores

* **deps:** update dependency lefthook to v2.2.1 ([491219d](https://github.com/getcihub/cihub/commit/491219dcd6a66fc8d169cb4fc26410f00992d4b3))
* **deps:** update dependency vitest to v5.0.4 ([df402cd](https://github.com/getcihub/cihub/commit/df402cd474476f57e779f18538356a7f07f368c7))
* **deps:** update github-actions ([559d8be](https://github.com/getcihub/cihub/commit/559d8be4653dcb65ff4a8722be67d633c7537398))
* **deps:** update rust crate proptest to v1.12.0 ([63b67fe](https://github.com/getcihub/cihub/commit/63b67fee36caf9178c269ff575f481bba9702acd))
* **deps:** update rust crate serde_json to v1.0.152 ([ff735df](https://github.com/getcihub/cihub/commit/ff735df415177d7eeb4d67c7d24104aeb67e4d38))
* **deps:** update rust crate serde_json to v1.0.154 ([73c0218](https://github.com/getcihub/cihub/commit/73c0218c92d315c90231911cd01b42e1f25e3786))
* **deps:** update rust crate tokio-util to v0.7.20 ([0cdf5dd](https://github.com/getcihub/cihub/commit/0cdf5dd5300c0a3d62980dc9eef5d9a433966013))

## [0.2.1](https://github.com/getcihub/cihub/compare/v0.2.0...v0.2.1) (2026-10-10)


### Bug Fixes

* **platform:** attach a repository to each connection that lists it ([f1ed32e](https://github.com/getcihub/cihub/commit/f1ed32ef359886fbf5cf89ed3aded551c124491d))


### Miscellaneous Chores

* **deps:** update dependency rust to v1.99.0 ([8d0a498](https://github.com/getcihub/cihub/commit/8d0a49890c512f78b99da1fb4333cc631e94b824))
* **deps:** update rust crate base64 to 0.23.0 ([bc05264](https://github.com/getcihub/cihub/commit/bc0526493250fe88576c98a11bff9359d8f95817))

## [0.2.0](https://github.com/getcihub/cihub/compare/v0.1.0...v0.2.0) (2026-10-10)


### Features

* **cli:** replace bootstrap with token create ([333ffcd](https://github.com/getcihub/cihub/commit/333ffcd94f179d7ae7b1e9b0b3c76bc883efd297))
* **config:** read the configuration file from .cihub/config.yaml too ([866ff04](https://github.com/getcihub/cihub/commit/866ff0436d7bd03a3e68ab20ae207f264f8cd364))
* **deploy:** add the Helm chart and its smoke test ([cf157ce](https://github.com/getcihub/cihub/commit/cf157ce5b0e9e54a57db734eb021741777c0ae6d))
* **mergequeue:** mention the author of a refused command in the answer ([1d30c66](https://github.com/getcihub/cihub/commit/1d30c6677446d86fb068b939466822953e69603c))
* **mergequeue:** show the history of the PR in its comment ([e941a09](https://github.com/getcihub/cihub/commit/e941a0905ec5172dd837c62e5c2789d04d45a398))
* **mergequeue:** show the test branch and the merged commit in the history ([3d82790](https://github.com/getcihub/cihub/commit/3d82790d13270fca582fe6466bfc9ba15e62371c))
* **platform:** sync the installations of the App ([571dc33](https://github.com/getcihub/cihub/commit/571dc333b6ec9fb19288fda5c7e4aac214907c31))
* **platform:** sync the installations of the App and onboard them without a bootstrap ([d5db5ad](https://github.com/getcihub/cihub/commit/d5db5ad4b28bd3f10803ee9970e4d4fda7ee7418))


### Bug Fixes

* **ci:** add inputs back ([86ab12d](https://github.com/getcihub/cihub/commit/86ab12de093b1df966afe412ed20d27ccd69985c))
* **ci:** use dedicated renovate github app ([b95dcf7](https://github.com/getcihub/cihub/commit/b95dcf7d9f7c0869603cda2e42f347018af56792))
* **ci:** use github app for release-please ([1ef667a](https://github.com/getcihub/cihub/commit/1ef667a292f115891681ae2ce7665c1eb77a15a2))
* **mergequeue:** refuse the submission of a PR that is merged, closed or a draft ([d9ea903](https://github.com/getcihub/cihub/commit/d9ea90317468939ef8a3a65b3ae5be9319345ecd))


### Code Refactoring

* **domain:** drop the enum values that no code constructs ([ad0e0ab](https://github.com/getcihub/cihub/commit/ad0e0ab86e9fad064106cbbe04414a9f3f2ce54c))
* **mergequeue:** compute the failure reason and the last report of a run when read ([6d396c6](https://github.com/getcihub/cihub/commit/6d396c673902d8331145055b627180ed375eb6ee))
* **mergequeue:** convert the errors of other crates with from ([eb7f46b](https://github.com/getcihub/cihub/commit/eb7f46bb0c5ac25c8e870a232107a12bd41f2ec4))
* **mergequeue:** drop org_modules, every organization has the merge queue ([7a98aac](https://github.com/getcihub/cihub/commit/7a98aac00d518b6051cfd002467093a6681a1a87))
* **mergequeue:** flatten the assembly and merge jobs ([c311106](https://github.com/getcihub/cihub/commit/c3111065fff01709b8b39a99ea65d8fbbc816493))
* **mergequeue:** keep the last snapshot of a queue in two columns of queues ([008b299](https://github.com/getcihub/cihub/commit/008b299e7aa2a510c04436640e183167f65d3f3a))
* **mergequeue:** make the items used in one module private ([67519d6](https://github.com/getcihub/cihub/commit/67519d6fa227616cd915ccc32350c60d140d125b))
* **mergequeue:** make the items used in one module private ([ade035c](https://github.com/getcihub/cihub/commit/ade035c89c834172becf2803f874a2bd9b578acd))
* **mergequeue:** remove one-caller job wrappers and flatten the merge command parser ([8714801](https://github.com/getcihub/cihub/commit/8714801255cdc61692c04ddbe546dae8404b562f))
* **mergequeue:** simplify the merge queue and its data model ([c2c43ff](https://github.com/getcihub/cihub/commit/c2c43ff49005d72a78df0636abccfce74d4e5660))
* **mergequeue:** split the scheduler loop and the submission job into named steps ([3294232](https://github.com/getcihub/cihub/commit/329423293137f8630df502877b30fdce2074f302))
* **mq-core:** remove dead values, a forwarder and a boolean parameter ([a8b3c3d](https://github.com/getcihub/cihub/commit/a8b3c3d917e479701aa2e2a06fd26cad3e4f54f3))
* **platform:** keep only the work to do in the inboxes ([cd8c088](https://github.com/getcihub/cihub/commit/cd8c0887f380c57f700132480bd8570a76cb6bb7))
* **platform:** keep only the work to do in the inboxes ([117d608](https://github.com/getcihub/cihub/commit/117d6084438b5968ea30b89b70ad598b87debe93))
* **platform:** keep the lease of a queue in two columns of queues ([b743741](https://github.com/getcihub/cihub/commit/b743741ada47a2530a8a6c2674a171be84633e49))
* **store:** drop memberships and three columns that no code reads ([6656c6b](https://github.com/getcihub/cihub/commit/6656c6bdd0d133fc14eac8d68c248f6eedb8f251))
* **store:** drop the projection columns that no code reads ([95110e7](https://github.com/getcihub/cihub/commit/95110e74ddba3e9a836588f945983b3dd67037f0))
* **store:** keep the foreign key from scm_connections to scm_instances ([0b1eb1f](https://github.com/getcihub/cihub/commit/0b1eb1f15a5bb4193f04b0d3b842e00bd703441d))
* **store:** merge scm_accounts into scm_connections ([d1182ad](https://github.com/getcihub/cihub/commit/d1182adda904c2c088d22a8390409c744f211846))
* **store:** serve a single SCM instance per deployment ([d55110b](https://github.com/getcihub/cihub/commit/d55110bc8f6ba487854a08f24dc44880c0bce349))
* **store:** serve a single SCM instance per deployment ([39799d3](https://github.com/getcihub/cihub/commit/39799d3d47316f10d3686d4bda8d237c0471110f))


### Documentation

* mark the done steps of the schema plan and count the tables ([aa29a84](https://github.com/getcihub/cihub/commit/aa29a848a8944fc15da71cdf0710aee642056935))
* record D246, a single SCM instance per deployment ([45948a1](https://github.com/getcihub/cihub/commit/45948a1a9625b2184e7ab75b637e447c99f73b4c))
* record D247 and D248 ([47bee72](https://github.com/getcihub/cihub/commit/47bee728f9b1e927989c77954f90a4cd40200826))
* record D249 ([dc19861](https://github.com/getcihub/cihub/commit/dc19861093d44d8c6d628daffe8f3d19617e390b))
* record D250, the readability rule ([7a0cb84](https://github.com/getcihub/cihub/commit/7a0cb84d583106e6b3c892b4ae8bccfb04408c86))
* say that repos and webhook_inbox lose scm_instance_id ([5f8be6a](https://github.com/getcihub/cihub/commit/5f8be6a32d0b0b60f4e78dc512be58884bc98f66))


### Styles

* **client:** separate the steps of the functions with blank lines ([b1d17b6](https://github.com/getcihub/cihub/commit/b1d17b63b1dbd83673673c092e5f0e716cc357ca))
* **cli:** separate the steps of the functions with blank lines ([98d69ca](https://github.com/getcihub/cihub/commit/98d69ca35a48a74bd60a5505138f3f0e6c4789b0))
* **conditions:** separate the steps of the functions with blank lines ([6b06957](https://github.com/getcihub/cihub/commit/6b06957a0427bcb357d3b03b1278229f0e789c12))
* **config:** separate the steps of the functions with blank lines ([b0b2745](https://github.com/getcihub/cihub/commit/b0b27458cd38a105b5ad66ad79867ed9a1672c73))
* **domain:** separate the steps of the functions with blank lines ([7bd8b93](https://github.com/getcihub/cihub/commit/7bd8b93725a15062733590dd39e9ed75086a8f13))
* **github:** separate the steps of the functions with blank lines ([316dbf2](https://github.com/getcihub/cihub/commit/316dbf276a78dcdb3747d82f7d8ef9d5de7c0f91))
* keep a let and its immediate use together ([3b4e68b](https://github.com/getcihub/cihub/commit/3b4e68be2ca4fe36f235357ebc723af86a683b2f))
* **mergequeue:** separate the steps of the functions with blank lines ([a95dfea](https://github.com/getcihub/cihub/commit/a95dfea7cae5b9f8a18c7af9f9e12b5330db5a3b))
* **mq-core:** separate the steps of the functions with blank lines ([def48db](https://github.com/getcihub/cihub/commit/def48dba280e40e89324bef35bcd9ad2b1eb9412))
* **platform:** separate the steps of the functions with blank lines ([0e1271b](https://github.com/getcihub/cihub/commit/0e1271b7b00124bf7c99946be2dd00212f27d961))
* **scm:** separate the steps of the functions with blank lines ([f42c5f6](https://github.com/getcihub/cihub/commit/f42c5f6ee7c491eb67d3eccd0ac141bbb2ba58dc))
* separate the steps of the functions with blank lines (D250) ([41f727c](https://github.com/getcihub/cihub/commit/41f727c4e9c823f42aabb79076cfe00851e8e064))
* **store:** separate the steps of the functions with blank lines ([5c0643d](https://github.com/getcihub/cihub/commit/5c0643ded1883234f2a817746fb3e0c912977cf2))
* **web:** separate the steps of the functions with blank lines ([9c37733](https://github.com/getcihub/cihub/commit/9c37733d0f4fa0f535154ed080147fe392aa11f0))


### Tests

* **github:** record the contract with the installations of the App ([d41944b](https://github.com/getcihub/cihub/commit/d41944b8f28d0f6d34835465b0a82a39f8373138))


### Continuous Integration

* run the CI in parallel jobs on runners hosted by GitHub ([3440728](https://github.com/getcihub/cihub/commit/3440728e1d6bb0796eb391aef2cd3a0a5344916c))


### Miscellaneous Chores

* add shellcheckrc configuration ([369bc35](https://github.com/getcihub/cihub/commit/369bc353d3b63022113290254c621d1fc53e2ffc))
* **ci:** update configuration for CI and release-please ([987ccc7](https://github.com/getcihub/cihub/commit/987ccc793b421df2018a4996cebe924a7592cddc))
* merge the queue main with squash again ([1f7ea9b](https://github.com/getcihub/cihub/commit/1f7ea9b8143745d581837d47fc71e01fc18b8b82))
* merge the stacked PRs with merge commits ([e66b8a0](https://github.com/getcihub/cihub/commit/e66b8a02e49810893211fed080f1d021757bac3a))
* move the image, the chart and the links to getcihub ([be4b769](https://github.com/getcihub/cihub/commit/be4b76997e52aa777ebf289521bb3a151e2d3cfd))
* **release:** bump minor on feat before 1.0 ([209c8db](https://github.com/getcihub/cihub/commit/209c8dbf0e9af06e10156b87430cfc25fcd6be4b))
* remove release-as from release-please config ([521f346](https://github.com/getcihub/cihub/commit/521f34686dd5aaa4281bc9b95dc43abcc82911f8))
* replace the decision log, check the rules of AGENTS.md, and rewrite the README ([#72](https://github.com/getcihub/cihub/issues/72)) ([dd59126](https://github.com/getcihub/cihub/commit/dd591269adbb9212b3a6e4702b0d2392abd02042))
* run no-comments ([9176321](https://github.com/getcihub/cihub/commit/91763212102062d12115942f72f132f23c7d0f80))
* update README and license ([74480dd](https://github.com/getcihub/cihub/commit/74480ddbc99886c808c4f123913263a6de0d5838))

## [0.1.0](https://github.com/nlecoy/cihub/compare/v0.1.0...v0.1.0) (2026-10-08)


### Features

* add the SCM contract, its simulator and the GitHub adapter ([113af4d](https://github.com/nlecoy/cihub/commit/113af4d02f7bc9a2dd8f4cca020b426c2b77ac68))
* assemble the test branches, conclude the runs and close lot 6 ([65d37cd](https://github.com/nlecoy/cihub/commit/65d37cdaa8a7707aa111eefb381fe3e4b2ff77d4))
* **cli:** one cihub binary with a server configuration file ([4766599](https://github.com/nlecoy/cihub/commit/4766599795d39191a70755d1b57e2995209c490c))
* **conditions:** add cihub-conditions crate ([9d42de3](https://github.com/nlecoy/cihub/commit/9d42de326a24825c4a59bc1b7a80371f90169771))
* **config:** add cihub-config crate ([c89c894](https://github.com/nlecoy/cihub/commit/c89c894df57ad0f14edea0cc4b724d7dff653926))
* **domain:** add cihub-domain crate ([d85fc9f](https://github.com/nlecoy/cihub/commit/d85fc9f2819b4b95adfe21b978d2b868081948d4))
* **domain:** add the closed vocabulary and the log events of lot 1 ([5f76edc](https://github.com/nlecoy/cihub/commit/5f76edcdd52c714a33f70afe3909e44922754c0d))
* ingest, route and sweep the SCM deliveries, and close lot 4 ([57e2791](https://github.com/nlecoy/cihub/commit/57e2791bb91b9a86cdfca72b0a8362a40ba3c79d))
* merge the PRs, repair lost webhooks and close lot 7 ([e9f015b](https://github.com/nlecoy/cihub/commit/e9f015b9ac542fd6e677cad9630696819db6a573))
* **mergequeue:** a lighter layout for the comments on the PR ([0061aa5](https://github.com/nlecoy/cihub/commit/0061aa51c8d5af4a432b0716cbb58b243ce4174e))
* **mergequeue:** acknowledge an accepted command with a reaction ([313c1e4](https://github.com/nlecoy/cihub/commit/313c1e4d357ae2a42582c4dac9156f7c868b1c20))
* **mergequeue:** batched strategy (lot 12) ([535d95c](https://github.com/nlecoy/cihub/commit/535d95c64dda5ca55f4371e75612ed422d2e6c67))
* **mergequeue:** batches, bisection, failure depth and optimistic merge ([c9302cb](https://github.com/nlecoy/cihub/commit/c9302cb671e86e68f3954114f44cea9525642292))
* **mergequeue:** create the queues at the first command ([0e044db](https://github.com/nlecoy/cihub/commit/0e044db94cee1f04b5e2c3712fdb1a3c799745ab))
* **mergequeue:** create the queues at the first command ([0ceb971](https://github.com/nlecoy/cihub/commit/0ceb9714d485d3acb601d0e0dabb9c962cce04cc))
* **mergequeue:** keep a PR out of the queue until it meets its conditions ([adb6ad1](https://github.com/nlecoy/cihub/commit/adb6ad103858285d631cf9902d4877b48fbbdb3b))
* **mergequeue:** link the test of a PR instead of naming its branch ([dc367d0](https://github.com/nlecoy/cihub/commit/dc367d02bfa8e8dfdfeb7ca91b5bedb40a6ef916))
* **mergequeue:** merge an up-to-date PR directly, off by default ([7678ad3](https://github.com/nlecoy/cihub/commit/7678ad3e4ff0f9d611dd10e0d5775e8886a883ac))
* **mergequeue:** name the draft PR of the test in the comment of the PR ([3d84a01](https://github.com/nlecoy/cihub/commit/3d84a0144217ee3f6aefd081cc69e2f638cbd589))
* **mergequeue:** read a waiting PR at once when a check concludes on its head ([9f285e1](https://github.com/nlecoy/cihub/commit/9f285e18462253abee6a6d21737a259c80e55c12))
* **mergequeue:** read the configuration file of each queue ([5bda346](https://github.com/nlecoy/cihub/commit/5bda3462a6e97c65fdb3e9524c8d906253041e7f))
* **mergequeue:** refuse submissions while draining and resubmit on push ([2165fcf](https://github.com/nlecoy/cihub/commit/2165fcf3da3e9a8039157974b02adeabf332fcfe))
* **mergequeue:** require the merge conditions and inject the branch rules ([a46bf9f](https://github.com/nlecoy/cihub/commit/a46bf9f85100e8594744a71c89c31bffe85dce3c))
* **mergequeue:** route a PR between the queues of its branch ([aa4d2ab](https://github.com/nlecoy/cihub/commit/aa4d2ab8de187d15f2b4e0aab7065b56035f98d5))
* **mergequeue:** route submissions with route_conditions ([09c8e9a](https://github.com/nlecoy/cihub/commit/09c8e9a877800aa16b08452be65870d21c8b2ec9))
* **mergequeue:** show when a waiting batch closes in the PR comment ([3c6a509](https://github.com/nlecoy/cihub/commit/3c6a50995645fc0ceb9a98a33f48e4ce42ead2bb))
* **mergequeue:** start a batch at once when a no_batch PR follows it ([e906af4](https://github.com/nlecoy/cihub/commit/e906af460de69421ab40b93fb2e4c7c3b4c60c67))
* **mq-core:** adapt the speculation window ([7e1197e](https://github.com/nlecoy/cihub/commit/7e1197e64ffc58725305ea1149a684637b004e3c))
* **mq-core:** add apply, the merge graph, plan and check ([890ba88](https://github.com/nlecoy/cihub/commit/890ba888458843eee0eea06d5d67d2de859f1b57))
* **mq-core:** add the simulate example ([70bd61e](https://github.com/nlecoy/cihub/commit/70bd61e6d040f4d0c51fb93b3ae09aa5b5bdef9b))
* **platform:** add the jobs, the worker loop and the daily maintenance ([f752e9c](https://github.com/nlecoy/cihub/commit/f752e9c59d9b05dfca38b9140f98fb741b91c438))
* **platform:** add the queue leases, the webhook inbox and the wake-up ([7f85bf8](https://github.com/nlecoy/cihub/commit/7f85bf8e768347f4b4b72dae2a462bff33db6d97))
* **platform:** add the transport example ([c03e9fd](https://github.com/nlecoy/cihub/commit/c03e9fd8ac0eb6b8f58501f4fe97ea518743b55e))
* **platform:** pause the jobs of a connection after a rate limit ([8719814](https://github.com/nlecoy/cihub/commit/87198141f5512f333413b3dad104f5f7b5dcee74))
* **platform:** stop a job after a timeout and close lot 3 ([120ebe5](https://github.com/nlecoy/cihub/commit/120ebe59f3ef42df087a7eb7463ae4680285783a))
* reconcile the queues, admit the PRs and close lot 5 ([4ba7efc](https://github.com/nlecoy/cihub/commit/4ba7efc08d8686af54153a04e811e5213bd19e4d))
* **server:** add the cihub-server skeleton ([34705c0](https://github.com/nlecoy/cihub/commit/34705c0f5c4e7ca1f2e750a1f1b4bd65ca8676bf))
* show a sentence for each reason on the PR, in the API and in the CLI ([bf6069d](https://github.com/nlecoy/cihub/commit/bf6069d1d2487fe88421341c6024db8b67f2ee0e))
* **store:** add cihub-store and the migrate command ([d732b0a](https://github.com/nlecoy/cihub/commit/d732b0a9f67cfd8993c769978b5469319a22f618))
* **store:** add the API tokens and close lot 2 ([792f748](https://github.com/nlecoy/cihub/commit/792f748007834265da05aa3d280af58ea10ee6bd))
* **store:** add the event log and its monthly partitions ([c1b9975](https://github.com/nlecoy/cihub/commit/c1b997523c70505a2ae47d2e6bc324fdc7b8ec97))
* **store:** add the tenancy tables and row level security ([4fee602](https://github.com/nlecoy/cihub/commit/4fee602a17d32decbf73b884cda40544b17e01ee))
* surfaces, API, CLI and image of lot 9 ([0a9fd4b](https://github.com/nlecoy/cihub/commit/0a9fd4ba42e02731522508dd6a6812b71f5e6065))
* **telemetry:** add cihub-telemetry for the logs ([e7ecc6b](https://github.com/nlecoy/cihub/commit/e7ecc6bf1199a64eb21fd05e9da43cff9fe65159))


### Bug Fixes

* **github:** merge a stacked PR through the asynchronous endpoint ([724222d](https://github.com/nlecoy/cihub/commit/724222d6c3b7ecad9bc8d2120ab9f4c87f6bf43b))
* **mergequeue:** give a shared push to each queue after a merge in flight ([70021c0](https://github.com/nlecoy/cihub/commit/70021c01a7fa3aea7c5dc9a2e5b5aaf106706174))
* **mergequeue:** log the answer of the SCM when a merge fails ([4c6b519](https://github.com/nlecoy/cihub/commit/4c6b519dfca1f34847c44b5f59e5bbbd4514d28d))
* **mergequeue:** read the time of a reconciliation once the queue is taken ([09e7ac0](https://github.com/nlecoy/cihub/commit/09e7ac033eda354cec80a27b000cfee171ce0c22))
* **platform:** lock only the claimed jobs so a concurrent worker sees the others ([ff7580a](https://github.com/nlecoy/cihub/commit/ff7580a4e265da6e57b2eddad4aa9a9118eaf9e7))
* **platform:** wake a scheduler at the next tick of its queues ([cbfd2ec](https://github.com/nlecoy/cihub/commit/cbfd2ecc4f0b8e8c5151d7ffa585efd9f4c853b1))
* update recorded contracts ([cd80e24](https://github.com/nlecoy/cihub/commit/cd80e24233a14b10fdc04c90dae9ce0e579f8f9f))


### Code Refactoring

* **config:** make .cihub.yaml the only source of the queue settings ([012d8d1](https://github.com/nlecoy/cihub/commit/012d8d1f76ca09e6ffc8d24909fcb19cceb7ce26))
* **domain:** forbid the clock and randomness outside one place ([7d484c4](https://github.com/nlecoy/cihub/commit/7d484c4374e066899612fbbe00da45b5fd703563))
* remove unused code and consolidate the migrations ([e1a8ac2](https://github.com/nlecoy/cihub/commit/e1a8ac2017a34c1bb775d80166bd6e83549e4c3a))
* **store:** make the tenancy model independent of the SCM ([9ad3311](https://github.com/nlecoy/cihub/commit/9ad3311d4caa0a5d3329faaa1248b0f392988309))
* **telemetry:** use the JSON format of tracing-subscriber ([43d29ad](https://github.com/nlecoy/cihub/commit/43d29ad5706ab8d75e998d350ddae87e0f8ea013))


### Documentation

* add project goal and specification ([45aa715](https://github.com/nlecoy/cihub/commit/45aa7155aa1972e34335df18a433709cb0ceb22d))
* **agents:** record the decisions D114 to D118 of lot 6 ([3aadb9b](https://github.com/nlecoy/cihub/commit/3aadb9b69ece69bc22e4a718bf05383635eeb108))
* **agents:** record the decisions D119 to D133 of lot 7 ([2e054cb](https://github.com/nlecoy/cihub/commit/2e054cb9885a3f6d62be76e63b136a7b672c29e3))
* **agents:** record the decisions D82 to D92 ([d87f930](https://github.com/nlecoy/cihub/commit/d87f930625f727d637a087c599c588847266ccf4))
* **agents:** record the decisions for lot 2 ([9a37e19](https://github.com/nlecoy/cihub/commit/9a37e19bdacc532edc5c04a76b75908c07d3b152))
* **agents:** record the decisions of lot 1 ([6a2af37](https://github.com/nlecoy/cihub/commit/6a2af371dd0faeea376df0c5d7c3f68e32e35eea))
* **agents:** record the decisions of lot 3 ([9b5c26b](https://github.com/nlecoy/cihub/commit/9b5c26b6455c4da44c285ec0ae1419f6eef99541))
* **agents:** record the decisions of lot 4 ([253e80c](https://github.com/nlecoy/cihub/commit/253e80c8d7360fc02ea49111e41fd3d758de54c8))
* **agents:** record the decisions of lot 5 ([287d347](https://github.com/nlecoy/cihub/commit/287d347a34a5f767a73296e0dc7e1a9b036bb441))
* **lots:** mark lot 0 as done ([7edb7e8](https://github.com/nlecoy/cihub/commit/7edb7e8da60aa38ba61b6711725316d02ea0a6f3))
* remove every compatibility with other products ([7b713a3](https://github.com/nlecoy/cihub/commit/7b713a3ff8c05f4233480d73d0475532040ce1d6))
* rewrite AGENTS.md and docs/ for the Rust implementation ([1d3cc74](https://github.com/nlecoy/cihub/commit/1d3cc74b53c924543b1f686cd15bcdf5d1a7000a))
* say connection instead of installation in the neutral documents ([1fd9a12](https://github.com/nlecoy/cihub/commit/1fd9a12b3ce51504ebce8bcf988f6331f824a8f7))
* settle the decisions of lot 12, the outbox and phase 1 ([766e66f](https://github.com/nlecoy/cihub/commit/766e66f81c7bb23175d8a64b89e77b2722f85c23))


### Tests

* **mergequeue:** a PR cannot change its own conditions, and close lot 8 ([a2875b6](https://github.com/nlecoy/cihub/commit/a2875b624eb964f3a0a4b87c51b6a014836cb44a))
* **mergequeue:** load of the batched strategy and fault injection ([61a310c](https://github.com/nlecoy/cihub/commit/61a310cfe5bdaf7c36a4bc3b40ced4911821b39f))
* **platform:** use from_mins for the clippy lint ([f8412e4](https://github.com/nlecoy/cihub/commit/f8412e44557a8001ada2c5667536b3d1d0b773a1))


### Build System

* **dev:** converge the development Postgres role with a setup service ([f811827](https://github.com/nlecoy/cihub/commit/f8118270886f5940b5c3e36bf8e2afa2db2a3f75))
* install cargo tools as prebuilt binaries ([b741558](https://github.com/nlecoy/cihub/commit/b741558a4ce66e63aa61f48e030d6ab190a78b50))
* install Rust and tools with mise, add a development Postgres ([d04a4b6](https://github.com/nlecoy/cihub/commit/d04a4b6d45829519fd922801f4044d809314d2c8))
* log the migrate task in the pretty format ([e2eafe3](https://github.com/nlecoy/cihub/commit/e2eafe32deffdb2a78e52a36a0db6c1e2e9f7b7b))


### Continuous Integration

* add GitHub Actions workflow ([510105e](https://github.com/nlecoy/cihub/commit/510105e945a7aa4cee7cf9c2723e4f9ec4bdd0cf))
* run the CI and the fault injection test on the self-hosted cihub-runner ([d4ee160](https://github.com/nlecoy/cihub/commit/d4ee1603426fba944ca58968f85c921fff89f348))
* run the CI and the fault injection test on the self-hosted cihub-runner ([84bfb63](https://github.com/nlecoy/cihub/commit/84bfb63f51e4a94ebfa54a3b4707055d9fea8f24))
* run the CI in one job, CI, and require it in .cihub.yaml ([#48](https://github.com/nlecoy/cihub/issues/48)) ([6bbddf7](https://github.com/nlecoy/cihub/commit/6bbddf7d8d2fd2d72063b792cb31f83e8d3cbf77))
* run the CI in two jobs for the two runners, with one build for all the tests ([b0982a4](https://github.com/nlecoy/cihub/commit/b0982a4240b767013e61deeb26f023b68d77f9f3))
* run the mise tasks, release with release-please, update with Renovate ([e51267b](https://github.com/nlecoy/cihub/commit/e51267b193a53e7a90f3fe9d8cdeaf15af8eb100))
* run the Rust checks in one job to share one build ([b1b708f](https://github.com/nlecoy/cihub/commit/b1b708f6e89720f90588f7a6eb7454c0a60b19ff))
* run the Rust jobs in parallel again, with the coverage in Rust Test ([dbb730e](https://github.com/nlecoy/cihub/commit/dbb730e490f35313fa85e3029587b2b7a2045176))
* select workspace crates in Cargo.lock without a method call ([da5c878](https://github.com/nlecoy/cihub/commit/da5c878722a18b3587e29e89db625020fe12e92d))
* skip incremental state and full debug information, and run 4 tests at a time ([73715b5](https://github.com/nlecoy/cihub/commit/73715b59d0b1670d6d07a895234604d861042262))
* update image name ([571a9e7](https://github.com/nlecoy/cihub/commit/571a9e721690b61bdf1c00387e4a433141a5693f))


### Miscellaneous Chores

* add Apache-2.0 license ([647310c](https://github.com/nlecoy/cihub/commit/647310cce8d74a6b4e31bc2011e9827154ca0678))
* add the CIHub configuration of the main queue ([03950f7](https://github.com/nlecoy/cihub/commit/03950f7c2337df26450733d4442a1c40bf428fd0))
* close LOT 12 ([c3ad44b](https://github.com/nlecoy/cihub/commit/c3ad44baafc50148fb91700de2848e850ce7089f))
* close LOT 9 ([58bccad](https://github.com/nlecoy/cihub/commit/58bccadf03c002ed6fe2ab9fe4b40f663b6bf0fd))
* **deps:** update dependency lefthook to v2.1.17 ([0a61c60](https://github.com/nlecoy/cihub/commit/0a61c60be6b3ea993717ca58231112a60e6228bd))
* **deps:** update github-actions ([96650c9](https://github.com/nlecoy/cihub/commit/96650c95913b191961cdd8bb6ffaf8baa071c7c5))
* **deps:** update github-actions ([22a8c5f](https://github.com/nlecoy/cihub/commit/22a8c5ffd2b1ce9e38faf31b4ddd350a658dd94e))
* **deps:** update github-actions ([9ee15f7](https://github.com/nlecoy/cihub/commit/9ee15f722767818758781fd282361ea0b23b7513))
* **deps:** update rust crate tokio to v1.53.2 ([ccacea7](https://github.com/nlecoy/cihub/commit/ccacea7f6b9ce2ba73772a26be83dccc6890f389))
* improve agentic flow ([7a0a269](https://github.com/nlecoy/cihub/commit/7a0a269ba15f203172354abdde16bcb22b6b0238))
* route hotfix/ branches to a sequential hotfix queue ([7227ef4](https://github.com/nlecoy/cihub/commit/7227ef4af2393ca94bd835e220cf9fe865391629))
* scaffold Rust workspace and tooling config ([44cba65](https://github.com/nlecoy/cihub/commit/44cba65fb665c12d9ff9cefcf2d017899f200918))
* stop testing before pushing ([2719b36](https://github.com/nlecoy/cihub/commit/2719b36e3e8c243f89435c74ed15122ea66cbc90))
* update release-please version ([1cd0109](https://github.com/nlecoy/cihub/commit/1cd0109bc8ce96b8294d10648b712861b652b7a6))
* use the batched strategy for the main queue ([193b53a](https://github.com/nlecoy/cihub/commit/193b53a48e6ddef41f1e00203924a498186ac79f))
