# Changelog

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
