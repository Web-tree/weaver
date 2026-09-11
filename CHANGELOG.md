# Changelog

## [0.2.0](https://github.com/Web-tree/weaver/compare/v0.1.0...v0.2.0) (2026-09-11)


### Features

* Add CLI commands (check, describe, list, module), integration tests, git-module example, and gap analysis specs ([05b352a](https://github.com/Web-tree/weaver/commit/05b352a72e5b4bd5a95c587d57325e92b02602cc))
* Add speckit agent workflows, a WASM Rust example, and update core components, logging, and an existing spec task file. ([ae88764](https://github.com/Web-tree/weaver/commit/ae8876470f537e732d3164a149472ded337d4537))
* **apply:** saved-plan replay with stale-plan detection ([cf5c389](https://github.com/Web-tree/weaver/commit/cf5c3890b18b283c98259ddcc7d782f7fc20c0d6))
* **cli:** add 'rw module add' to adopt and pin a module ([54b607f](https://github.com/Web-tree/weaver/commit/54b607f850aa68648e3b3fff77c9eec4a5868af2))
* **cli:** add self-update from GitHub releases ([08be5be](https://github.com/Web-tree/weaver/commit/08be5be69703845e9411be49c2db72ea58556680))
* **cli:** dispatch app-level file ensures through the Ensure trait ([d666195](https://github.com/Web-tree/weaver/commit/d666195f42862d1375f7fde2ad3b6f1a81682214))
* **cli:** structured plan changes and typed exit codes ([ae136e0](https://github.com/Web-tree/weaver/commit/ae136e0cc498bb7c0885191f22522aa7c0589d49))
* **core:** add ensure.file.exists primitive ([2825a9d](https://github.com/Web-tree/weaver/commit/2825a9d5303a3df68537548bfcc6c078342d3f26))
* **core:** add ensure.file.from_template primitive ([b662e10](https://github.com/Web-tree/weaver/commit/b662e105ade26a6213c782a1b6bb7a8e63076ca6))
* **core:** add ensure.file.md_section with block_marker and heading selectors ([815ef3a](https://github.com/Web-tree/weaver/commit/815ef3a67552d54edff42161dd9e861f18b98f14))
* **core:** add sub-file owned_regions to FileState (additive) ([42a0b49](https://github.com/Web-tree/weaver/commit/42a0b49c45a0b862c9a74b8eef0e766d9d7ade6b))
* **core:** enrich EnsureContext with module_path and tera_context ([80dcbc3](https://github.com/Web-tree/weaver/commit/80dcbc33ce24f9633aef2e94ac261f0ca0882d46))
* **core:** extract generic ensure_json_key; npm ensures become presets ([374db06](https://github.com/Web-tree/weaver/commit/374db060673988c15c2139d591ee8054e7e7e541))
* **core:** pin module refs to commits and wire the module lockfile ([81a709a](https://github.com/Web-tree/weaver/commit/81a709a8a08dd447586b84698c7512446275bc7b))
* **core:** treat missing module manifest as empty ([7b82c60](https://github.com/Web-tree/weaver/commit/7b82c606afee97b698bd212b60cb8b006d805308))
* **ensures:** npm ensures convergence actions + wasmtime 44 fix ([a58a79e](https://github.com/Web-tree/weaver/commit/a58a79eb182e9e6c529accaf5e0dcb34813f7c95))
* implement core MVP gaps — type checking, file copy, config includes, answers persistence ([#4](https://github.com/Web-tree/weaver/issues/4)) ([b06892f](https://github.com/Web-tree/weaver/commit/b06892f4ae9ae28b5c2041a75197494b45698f1e))
* Initialize repo-weaver project with core application structure, MVP specifications, and agent-driven workflows. ([2f192cb](https://github.com/Web-tree/weaver/commit/2f192cbb338a91b7fb8c8f07bab9cbd8ac08b1c0))
* Introduce `init` and `plan` commands, `speckit` agent workflows, and project initialization logic. ([4696c4f](https://github.com/Web-tree/weaver/commit/4696c4f5e1d22582b8c75bdc1c7226bc32692964))
* **plugins:** fs.symlink ensure plugin + resolver fix for local/git plugin overrides ([#90](https://github.com/Web-tree/weaver/issues/90)) ([a06e5d3](https://github.com/Web-tree/weaver/commit/a06e5d38cb911aeb0122989a79848d2cc1a7a0c9))
* **plugins:** integrate WASM plugin system for module ensures ([ea8252d](https://github.com/Web-tree/weaver/commit/ea8252d64348a9d5f15d1359f1b9ac5295e4116b))
* **plugins:** wire secrets, ai.patch, npm convergence + ecosystem plugins ([e571933](https://github.com/Web-tree/weaver/commit/e57193358810dada23d15fb4a221e726277e5b30))
* **release:** publish verified binaries and a curl|sh installer ([6ece6ac](https://github.com/Web-tree/weaver/commit/6ece6ac5ac874523c3ee0bffc7f7d62d9966013f))
* **state:** versioned state schema + honor stop strategy on drift ([8a03384](https://github.com/Web-tree/weaver/commit/8a03384fba80a057c670db33e9b6ae5e15ff2a02))
* **template:** support list inputs + jinja-style block trimming ([7a1f2e2](https://github.com/Web-tree/weaver/commit/7a1f2e2c4bc28284b6e5ffd1c69cd229b181bba2))
* **weaver:** rebrand directory configuration tool with wvr CLI ([5f282de](https://github.com/Web-tree/weaver/commit/5f282debb8bd33f7a3d000eacdc17dca9d8fc2d8))


### Bug Fixes

* **cli:** make 'rw module add' idempotent (update existing entry) ([5e816c3](https://github.com/Web-tree/weaver/commit/5e816c3dcddaceee6d3bc49a8c2e1e4d9306274f))
* **deps:** update rust crate reqwest to 0.13.0 ([#43](https://github.com/Web-tree/weaver/issues/43)) ([04e31b4](https://github.com/Web-tree/weaver/commit/04e31b404cd06ec1893edba705c55b0d8242130b))
* **deps:** update rust crate wit-bindgen to 0.61.0 ([#38](https://github.com/Web-tree/weaver/issues/38)) ([50bf23f](https://github.com/Web-tree/weaver/commit/50bf23fda1fb5596ec9bb1847c3f869b702bda88))


### Documentation

* **004:** add generic standardization engine spec + Phase 0/1 plan ([8dbb68e](https://github.com/Web-tree/weaver/commit/8dbb68e72346bc3d69a78be4cbd39a68bbc79568))
* **004:** record Phase 0+1 follow-ups from final review ([ad5c901](https://github.com/Web-tree/weaver/commit/ad5c901f2237b6830e49620bc21c7a5c46248d78))
* add Rust code quality validation checklist ([fa1352c](https://github.com/Web-tree/weaver/commit/fa1352c4032c1acbd7cdd38560bae2dbc7c5301c))
* **beads:** design spec for beads module (beads-first, extract-later) ([b1f904c](https://github.com/Web-tree/weaver/commit/b1f904cd9e3a10973a5b5bbaacd90c4e01017074))
* **beads:** move module out of repo-weaver; keep examples/ infra-agnostic ([606708e](https://github.com/Web-tree/weaver/commit/606708ed76a541adae56937bf9d3e785f2b325e4))
* document installing and updating wvr ([8f15f5a](https://github.com/Web-tree/weaver/commit/8f15f5af6af2a2e3cd08f48d7c77b9bbcbc904c8))
* **marketplace:** add weaver marketplace design; shape beads to fit ([64d3d87](https://github.com/Web-tree/weaver/commit/64d3d87756fd441d617428b640c077875dce9e8c))
* **specs:** plugin-first standards engine design and issue catalog ([#85](https://github.com/Web-tree/weaver/issues/85)) ([6452d35](https://github.com/Web-tree/weaver/commit/6452d3579ef214860b098bbe3d86f526d5fc4ebd))
