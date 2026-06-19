# Third-Party Notices

AgentJS currently uses Boa as its ECMAScript parser, virtual machine, runtime,
and built-in implementation through local path dependencies under `boa/`.

- Project: Boa
- Source: <https://github.com/boa-dev/boa>
- License: MIT OR Unlicense
- License texts: `boa/LICENSE-MIT` and `boa/LICENSE-UNLICENSE`

The `quickjs/` and `test262/` directories are reference/test repositories and
retain their own copyright and license files. QuickJS is not linked into the
AgentJS binary. Test262 test data is used only by the conformance runner.

