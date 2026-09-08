# Weaver project description and requirements

## 1. Purpose

Weaver is a declarative tool for configuring and maintaining directories and workspaces in a consistent way across many stacks (TypeScript, Go, Rust, Terraform/OpenTofu, Kubernetes YAML, GitHub Actions, Taskfile, and others). Repositories and monorepos are important workspace targets, alongside home directories and other managed directories.

It solves two problems:

1. Bootstrap: generate a working project or workspace quickly with prompts for required parameters.
2. Update: re-run later when upstream modules change (new variables, new tasks, new conventions) and converge the target directory to the desired state again.

Weaver must be able to run repeatedly and be idempotent.

## 2. Key principles

1. Declarative desired state
   The YAML describes the state the target directory should have. Running Weaver converges actual state to desired state.

2. Native tool first
   Weaver must not implement custom parsers for external ecosystems when a native tool can be used instead.
   Examples:

* Taskfile: use `task -l` to inspect tasks
* npm: use `npm pkg get` and `npm pkg set` to inspect and mutate scripts/deps
* Go: use `go list`, `go mod edit`, `go get`, `go mod tidy`
* Rust: use `cargo` (and optionally cargo-edit if present)
* Terraform/OpenTofu: use `terraform/tofu` commands including `output -json`
* Kubernetes: use `kubectl -o json`, `kustomize`, `helm`

Weaver may parse only:

* its own YAML configs
* JSON outputs produced by tools
* simple regex captures from tool output when JSON is unavailable

3. Composition and reuse
   Support “apps from apps” by building blocks (modules) that can be combined to form more complex apps.

4. Updates without re-copying everything
   Updating a module or template should not require recreating a repo. It should update in place, prompt for new required inputs, and keep local customizations safe.

5. Deterministic by default, AI as a controlled option
   Most changes should be done via deterministic actions. AI-based changes are supported for complex edits, but must be controlled, verifiable, and rollback-safe.

## 3. Scope of what Weaver manages

Weaver manages:

* repository and monorepo scaffolding
* folder structure conventions
* wrapper Taskfile generation and task namespacing
* config files generation from templates (tfvars, yaml, env files, README fragments)
* pinned vendoring of upstream modules (git submodule or pinned clone)
* pipeline tasks that orchestrate native tools in steps and pass outputs between steps
* optional AI patch steps (unified diff, apply, verify, rollback)

Weaver does not need to:

* become a full CI system
* replace Terraform/ArgoCD/Helm/etc
* implement a full programming language template engine beyond standard templating
* parse and fully understand every ecosystem file format

## 4. Core concepts

### 4.1 Workspace

A workspace is a directory containing one root config file and optionally additional config fragments.

* Root file: `weaver.yaml`
* Optional directory: `weaver.d/` with multiple YAML fragments
* Optional app-local config files inside app folders

### 4.2 App

An app is a unit of configuration bound to a folder path (monorepo support).

Each app declares:

* path: folder where actions execute
* variables: user-provided or derived parameters
* ensures: declarative convergence actions
* tasks: pipelines for operational flows (install, plan, apply, destroy, etc)
* checks: validation commands

Apps can be:

* a single repo (path: ".")
* a monorepo app (path: "apps/web")
* an infra workspace (path: "clusters/prod")

### 4.3 Module

A module is a reusable building block package that can be imported and reused.

A module can be:

* local path
* git repository pinned to a tag/commit

A module may export:

* templates (files to render)
* blocks (named fragments that apps can extend)
* predefined ensures and tasks
* optional helper scripts used by deterministic actions
* optional prompts schema for variables

Modules must be versionable and pin-able.

### 4.4 Ensure

An ensure is a convergence action. It makes sure some state exists.

Every ensure has:

* type: identifies the built-in action
* parameters: inputs and desired state
* optional detect: how to detect current state (via native tools)
* optional verify: how to validate success (native tools)

Ensures must be idempotent. Re-running should result in no changes when state is already correct.

### 4.5 Task (pipeline)

A task is an executable pipeline of steps. It is not a convergence action, it is an operational flow.

Examples:

* k3s-nebula install pipeline: plan infra, apply infra, fetch outputs, apply config
* destroy pipeline: remove config, destroy infra
* release pipeline: bump versions, run tests, publish artifacts

Tasks must support:

* step ordering
* capturing outputs from one step and passing to others
* env and args templating using captured outputs
* optional stdin piping from one step to another
* readable logs and failure handling

### 4.6 AI patch step

A controlled step for complex modifications.

Requirements:

* Uses an external AI CLI tool (claude code, gemini cli, codex cli, or custom command)
* The AI must output only a unified diff
* wvr applies diff using git tooling (git apply) or patch
* wvr runs verify checks
* if verify fails, wvr rolls back changes

AI steps are optional and should be used only when deterministic ensures are insufficient.

## 5. Configuration requirements

### 5.1 YAML structure

The root configuration must support:

* version
* includes: globs or explicit files to load and merge (tree-like config)
* modules: list of modules with source and pinned ref
* apps: map of app definitions

Example conceptual structure:

* version: 1
* includes:

  * weaver.d/**/*.yaml
* modules: <name>:
  source: git:...
  ref: vX.Y.Z or commit
* apps: <appName>:
  path: <folder>
  extends: [module.block, ...]
  vars: { ... }
  ensures: [ ... ]
  tasks: { ... }
  checks: [ ... }

### 5.2 Tree-like configs and recursion

wvr must support:

* loading multiple YAML files via includes
* app configs that can live in subfolders and be discovered recursively (optional mode)
* deep merge of maps (vars)
* concatenation of arrays (ensures, checks, task steps)
* deterministic override rules (later files override earlier ones)

### 5.3 Variables and prompting

wvr must support variables from:

* YAML literal values
* environment variables
* derived values from tool outputs (captured during tasks or ensures)
* user prompts when missing required variables

It must store answers to allow future updates without asking again.
Example: `.rw/answers.yaml` per app or per workspace.

When module updates introduce new required vars:

  * wvr prompts for them on next run
  When vars are removed:
  * wvr warns and offers to keep or delete stored answers

### 5.4 Templates

wvr must support file rendering from templates with:

* access to vars
* access to module metadata
* access to computed values
* basic conditionals and loops

The templating engine can be a standard one (for example Jinja-like behavior). It must be deterministic.

## 6. Ensures library (minimum required)

weaver must ship with a core set of ensures. Initial set:

Repository structure and vendoring:

* ensure.folder.exists (create folder)
* ensure.file.from_template (render file)
* ensure.file.copy (copy static file)
* ensure.git.submodule (ensure submodule at path pinned to ref)
* ensure.git.clone_pinned (alternative to submodule)

Taskfile support:

* ensure.task.wrapper (generate wrapper Taskfile that includes upstream Taskfile)
* ensure.task.namespace_alias (optional convenience, implemented as wrapper tasks)

Node/npm support (native tool based):

* ensure.npm.script (ensure package.json script exists with value, using `npm pkg set`)
* ensure.npm.dep (ensure dependency version, using `npm pkg set` or `npm install`)
* ensure.npm.devDep (same)
* ensure.npm.engine (optional)

Go support:

* ensure.go.module_dep (ensure module version, using `go get module@version`)
* ensure.go.tidy (run `go mod tidy`)

Rust support:

* ensure.cargo.dep (ensure dependency version, prefer `cargo add` if available, fallback to AI patch or documented requirement)
* ensure.cargo.fmt (cargo fmt)

Terraform/OpenTofu support:

* ensure.tf.vars_file (render tfvars, run fmt if relevant)
* ensure.tf.init (terraform/tofu init with backend args)
* ensure.tf.validate

Kubernetes support:

* ensure.kubectl.apply (apply a rendered manifest or kustomize build output)
* ensure.kustomize.resource (use `kustomize edit` if kustomize exists)
* ensure.helm.release (optional)

AI:

* ensure.ai.patch (unified diff, apply, verify, rollback)

Markdown (partial updates):

* ensure.file.md_section (manage a named region of a Markdown file via a pluggable `selector`: `heading` path or `block_marker` id in v1; reserved: `mdast`, `mdq`, `regex`, `frontmatter`, `line_range`). Converges only the selected region; all other content in the file is preserved and drift-detected independently.

All ensures must have:

* plan capability: show what would change
* apply capability: make changes

## 7. Task pipelines requirements

### 7.1 Pipeline step schema

A pipeline is an ordered list of steps. Each step supports:

* id: unique step identifier
* cmd: command array to execute
* cwd: optional working dir relative to app path
* env: environment variables (templated)
* stdin: optional string (templated) for piping
* capture:

  * json: JSONPath-like selectors to extract values from stdout
  * regex: named captures from stdout
* outputs: resulting key-value map available to later steps
* allow_failure: optional
* timeout: optional

### 7.2 Step outputs model

After execution each step provides:

* stdout, stderr, exitCode
* outputs: from capture
* artifacts: optional file paths created or referenced

Later steps can reference:

* vars.<name>
* steps.<id>.stdout
* steps.<id>.outputs.<key>

### 7.3 Task composition

Tasks must support:

* calling another task (call)
* importing tasks from modules
* overriding or extending module tasks per app

Example:

* task install calls task infra then task config

### 7.4 Error handling modes

Minimum:

* fail fast (default)
  Optional modes:
* continue on error
* run from step id
* dry run (print commands)

## 8. CLI requirements (`wvr`)

Weaver must provide:

Config and discovery:

* `wvr list` shows apps and tasks
* `wvr describe <app>` shows resolved config (after includes and extends)

Convergence:

* `wvr plan [app]` plans ensures
* `wvr apply [app]` applies ensures
* `wvr check [app]` runs checks

Tasks:

* `wvr task list [app]`
* `wvr run <app> <taskName>` executes pipeline task
* optional: `wvr run <app> <taskName> --from <stepId>`

Modules:

* `wvr module list`
* `wvr module update <name> --ref <newRef>` updates pinned ref in config (or in a lock file) and optionally runs apply

Prompting:

* if required vars missing, prompt and store in answers file

Logging:

* human readable logs by default
* optional JSON logs for CI

Exit codes:

* non-zero on failures
* zero when converged and tasks succeeded

## 9. Updates and safety requirements

1. Idempotency
   Repeated runs must converge to the same state.

2. Local customization safety

* wvr should clearly define which files it owns (managed files)
* managed files can be overwritten on apply
* unmanaged files must not be changed unless explicitly targeted

3. Drift detection
   Plan mode must highlight:

* files that will change
* commands that will run
* module ref differences

4. Locking and reproducibility
   Support a lock mechanism for module refs and resolved versions:

* either inline pinned refs only
* or a generated lock file (recommended) like `.rw/lock.yaml`

5. AI patch safety

* require git repo or require wvr to initialize a git repo for safe rollback
* verify must run after patch
* rollback on failure is mandatory
* store the AI prompt and tool used in logs for auditability

## 10. Requirements specific to k3s-nebula usage

Weaver must support a pattern where:

* upstream `k3s-nebula` is vendored (submodule or pinned clone)
* a workspace wrapper is generated that contains:

  * infra and config tfvars folders (names configurable)
  * a wrapper Taskfile that includes upstream Taskfile and adds local alias tasks
  * an install pipeline task that runs:

    1. plan infra
    2. apply infra
    3. capture terraform outputs (prefer `terraform output -json`)
    4. apply config using outputs (KUBECONFIG path or API endpoint)

It must be possible to update `k3s-nebula` and then re-run:

* wvr prompts for new tf vars if needed
* regenerated wrapper tasks reflect new upstream tasks or renamed conventions

## 11. Non-goals (explicit)

* Do not build a new Terraform, Kubernetes, or package manager.
* Do not parse external config formats as the primary mechanism.
* Do not silently change user code outside declared ensures/tasks.
* Do not require always-on services. It should be usable as a CLI in local dev and CI.

## 12. MVP definition (what to build first)

MVP must include:

* YAML loader with includes and merge rules
* apps with path scoping
* modules pinned by git ref (submodule or clone pinned)
* answers storage and prompting for missing vars
* ensures:

  * folder.exists
  * file.from_template
  * git.submodule (or clone pinned)
  * npm.script (npm pkg set)
  * ai.patch (diff, verify, rollback)
  * task.wrapper (generate wrapper Taskfile)
* plan and apply for ensures
* pipeline tasks with output passing:

  * cmd execution
  * json capture
  * env templating
* CLI commands:

  * wvr list
  * wvr plan
  * wvr apply
  * wvr run

First real-world target:

* a k3s-nebula wrapper workspace that supports init, apply, update, and install pipeline.

## 13. Suggested implementation constraints

* single static binary preferred (Go or Rust are good fits)
* cross-platform where possible (macOS, Linux; Windows optional if desired)
* plugin system is optional for MVP, but design the YAML types so new ensures can be added cleanly later
* keep configuration schema versioned

## 14. Acceptance criteria

Weaver is acceptable when:

1. You can run `wvr apply` in an empty folder and get a fully working k3s-nebula workspace wrapper with generated tfvars and wrapper Taskfile.
2. You can update the pinned k3s-nebula ref, run `wvr apply` again, and Weaver:

* detects differences
* prompts for new required vars
* updates managed files safely

3. You can run `wvr run <app> install` and it:

* applies infra
* captures outputs from terraform output -json
* passes outputs into config apply step

4. You can add a new module that introduces a new task and ensure type without changing existing app configs beyond referencing the module.
