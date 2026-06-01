//! Validation tests for Group C sculptor rules ported as tree-sitter AST queries.
//!
//! Each rule covers match_examples and non_match_examples from sculptor's
//! ratchet_rules.py to ensure the tree-sitter form matches the original regex intent.

#![cfg(feature = "lang-python")]

mod sculptor_common;

use sculptor_common::{
    expect_match, expect_no_match, load_rule, load_rule_with_python_tests, matches,
};

// --------------------------------------------------------------------------
// no-bare-exit (sculptor: non_sys_exit)
// --------------------------------------------------------------------------
#[test]
fn no_bare_exit_matches() {
    let rule = load_rule("no-bare-exit");
    expect_match(&rule, "exit(0)\n", "exit(0)");
    expect_match(&rule, "exit(1)\n", "exit(1)");
    expect_match(&rule, "exit(255)\n", "exit(255)");
}

#[test]
fn no_bare_exit_non_matches() {
    let rule = load_rule("no-bare-exit");
    expect_no_match(&rule, "sys.exit(0)\n", "sys.exit(0)");
    expect_no_match(&rule, "sys.exit(1)\n", "sys.exit(1)");
    expect_no_match(&rule, "sys.exit(255)\n", "sys.exit(255)");
}

// --------------------------------------------------------------------------
// no-typing-cast (sculptor: cast)
// --------------------------------------------------------------------------
#[test]
fn no_typing_cast_matches() {
    let rule = load_rule("no-typing-cast");
    expect_match(
        &rule,
        "lambda a: cast(TaskDataModelService, _test_data_model_service)\n",
        "bare cast in lambda",
    );
    expect_match(&rule, "x = cast(str, value)\n", "bare cast assignment");
    expect_match(
        &rule,
        "for base in cast(tuple, cls.__orig_bases__):\n    pass\n",
        "bare cast in for",
    );
    expect_match(&rule, "return typing.cast(str, 'hello')\n", "typing.cast");
}

#[test]
fn no_typing_cast_non_matches() {
    let rule = load_rule("no-typing-cast");
    expect_no_match(&rule, "forecast(weather)\n", "forecast");
    expect_no_match(&rule, "self._cast(prev_action)\n", "_cast attribute");
    expect_no_match(&rule, "something.cast(x)\n", "non-typing attribute cast");
    expect_no_match(
        &rule,
        "Secret.from_dict(safe_cast(dict, secrets))\n",
        "safe_cast",
    );
}

// --------------------------------------------------------------------------
// no-unnumbered-pyre-ignore
// --------------------------------------------------------------------------
#[test]
fn pyre_ignore_unnumbered_matches() {
    let rule = load_rule("no-unnumbered-pyre-ignore");
    expect_match(&rule, "# pyre-ignore foo\nx = 1\n", "bare");
    expect_match(&rule, "# pyre-ignore: foo\nx = 1\n", "bare with colon");
    expect_match(&rule, "# pyre-ignore\nx = 1\n", "bare only");
}

#[test]
fn pyre_ignore_unnumbered_non_matches() {
    let rule = load_rule("no-unnumbered-pyre-ignore");
    expect_no_match(&rule, "# pyre-ignore[1] foo\nx = 1\n", "numbered");
    expect_no_match(&rule, "# pyre-ignore[1]: foo\nx = 1\n", "numbered colon");
    expect_no_match(&rule, "# pyre-ignore[1]\nx = 1\n", "just [1]");
    expect_no_match(&rule, "# pyre-ignore[10] foo\nx = 1\n", "[10]");
    expect_no_match(&rule, "# pyre-ignore-all-errors\nx = 1\n", "all-errors");
    expect_no_match(
        &rule,
        "# pyre-ignore-all-errors[1]\nx = 1\n",
        "all-errors[1]",
    );
    expect_no_match(&rule, "# something pyre-ignore\nx = 1\n", "embedded");
    expect_no_match(&rule, "# pyre-ignore[7, 19]\nx = 1\n", "multi-numbered");
}

// --------------------------------------------------------------------------
// no-unnumbered-pyre-fixme
// --------------------------------------------------------------------------
#[test]
fn pyre_fixme_unnumbered_matches() {
    let rule = load_rule("no-unnumbered-pyre-fixme");
    expect_match(&rule, "# pyre-fixme foo\nx = 1\n", "bare");
    expect_match(&rule, "# pyre-fixme: foo\nx = 1\n", "bare colon");
    expect_match(&rule, "# pyre-fixme\nx = 1\n", "bare only");
}

#[test]
fn pyre_fixme_unnumbered_non_matches() {
    let rule = load_rule("no-unnumbered-pyre-fixme");
    expect_no_match(&rule, "# pyre-fixme[1] foo\nx = 1\n", "[1] foo");
    expect_no_match(&rule, "# pyre-fixme[1]: foo\nx = 1\n", "[1]: foo");
    expect_no_match(&rule, "# pyre-fixme[1]\nx = 1\n", "[1]");
    expect_no_match(&rule, "# pyre-fixme[10]\nx = 1\n", "[10]");
    expect_no_match(&rule, "# something pyre-fixme\nx = 1\n", "embedded");
    expect_no_match(&rule, "# pyre-fixme[7, 19]\nx = 1\n", "multi");
}

// --------------------------------------------------------------------------
// no-unlabeled-type-ignore
// --------------------------------------------------------------------------
#[test]
fn type_ignore_unlabeled_matches() {
    let rule = load_rule("no-unlabeled-type-ignore");
    expect_match(&rule, "x = 1  # type: ignore\n", "bare");
    expect_match(&rule, "x = 1  # type: ignore foo\n", "bare foo");
    expect_match(&rule, "x = 1  # type: ignore: foo\n", "bare colon");
}

#[test]
fn type_ignore_unlabeled_non_matches() {
    let rule = load_rule("no-unlabeled-type-ignore");
    expect_no_match(
        &rule,
        "x = 1  # type: ignore[prop-decorator]\n",
        "labeled prop",
    );
    expect_no_match(
        &rule,
        "x = 1  # type: ignore[return-value]: foo\n",
        "labeled return",
    );
    expect_no_match(&rule, "x = 1  # type: ignore[1]\n", "labeled [1]");
    expect_no_match(&rule, "x = 1  # type: ignore[10]\n", "labeled [10]");
    expect_no_match(&rule, "x = 1  # something type: ignore\n", "embedded");
}

// --------------------------------------------------------------------------
// no-untyped-args-kwargs (sculptor: args_kwargs)
// --------------------------------------------------------------------------
#[test]
fn args_kwargs_matches() {
    let rule = load_rule("no-untyped-args-kwargs");
    expect_match(
        &rule,
        "def render(self, *args, **kwargs):\n    pass\n",
        "untyped",
    );
    expect_match(
        &rule,
        "def extend(self, *args, **kwargs) -> None:\n    pass\n",
        "untyped typed return",
    );
    expect_match(
        &rule,
        "def not_paramspec(self, *args: Paargs) -> None:\n    pass\n",
        "mistyped args",
    );
    expect_match(
        &rule,
        "def use(*args: Callable[..., Any]) -> Any:\n    pass\n",
        "args with Callable",
    );
    expect_match(
        &rule,
        "def params(cls, **kwargs) -> Foo:\n    pass\n",
        "untyped kwargs",
    );
}

#[test]
fn args_kwargs_non_matches() {
    let rule = load_rule("no-untyped-args-kwargs");
    expect_no_match(
        &rule,
        "def meta(*args: P.args) -> None:\n    pass\n",
        "typed args",
    );
    expect_no_match(
        &rule,
        "def meta(**kwargs: P.kwargs) -> None:\n    pass\n",
        "typed kwargs",
    );
    expect_no_match(
        &rule,
        "def meta(*args: P.args, **kwargs: P.kwargs) -> None:\n    pass\n",
        "both typed",
    );
}

#[test]
fn args_kwargs_counts_per_splat() {
    // Bead code-xep: `def f(*args, **kwargs)` is two independent fixes (annotate
    // each splat). Per-splat semantics emits one violation per offending node,
    // not a single violation on the shared `(parameters)` node. Regression
    // guard against the prior shape where both alternatives captured the same
    // `(parameters)` node and emitted byte-identical duplicate violations.
    let rule = load_rule("no-untyped-args-kwargs");
    assert_eq!(
        matches(&rule, "def f(*args, **kwargs):\n    pass\n"),
        2,
        "untyped *args and **kwargs should produce two distinct violations",
    );
    assert_eq!(
        matches(&rule, "def f(*args: Any, **kwargs: Any):\n    pass\n"),
        2,
        "mistyped *args and **kwargs should produce two distinct violations",
    );
    assert_eq!(
        matches(
            &rule,
            "def f(*args: P.args, **kwargs: P.kwargs):\n    pass\n"
        ),
        0,
        "both properly typed should produce no violations",
    );
}

#[test]
fn args_kwargs_catches_multiline_signature() {
    // Bead code-xep: sculptor's regex uses `.*` which does not span newlines,
    // so it misses untyped splats in multi-line signatures. Our AST query
    // catches them. Examples mirror real sculptor codebase occurrences.
    let rule = load_rule("no-untyped-args-kwargs");
    expect_match(
        &rule,
        "def inject_exception_and_log(\n    exc: BaseException, message: str, *args: Any, **kwargs: Any\n) -> None:\n    pass\n",
        "multi-line signature with mistyped splats",
    );
    expect_match(
        &rule,
        "def log_exception(\n    exc: BaseException,\n    message: str,\n    *args: Any,\n    **kwargs: Any,\n) -> None:\n    pass\n",
        "multi-line signature one-arg-per-line",
    );
}

// --------------------------------------------------------------------------
// classmethod-builder-naming (sculptor: non_build_classmethods)
// --------------------------------------------------------------------------
#[test]
fn classmethod_builder_matches() {
    let rule = load_rule("classmethod-builder-naming");
    expect_match(
        &rule,
        "class A:\n    @classmethod\n    def something(cls):\n        pass\n",
        "non-prefixed",
    );
    expect_match(
        &rule,
        "class A:\n    @classmethod\n    def create(cls):\n        pass\n",
        "create",
    );
}

#[test]
fn classmethod_builder_non_matches() {
    let rule = load_rule("classmethod-builder-naming");
    expect_no_match(
        &rule,
        "class A:\n    @classmethod\n    def build(cls):\n        pass\n",
        "build",
    );
    expect_no_match(
        &rule,
        "class A:\n    @classmethod\n    def build_from(cls):\n        pass\n",
        "build_",
    );
    expect_no_match(
        &rule,
        "class A:\n    @classmethod\n    def from_something(cls):\n        pass\n",
        "from_",
    );
    expect_no_match(
        &rule,
        "class A:\n    @classmethod\n    def load(cls):\n        pass\n",
        "load",
    );
    expect_no_match(
        &rule,
        "class A:\n    @classmethod\n    def _build(cls):\n        pass\n",
        "_build",
    );
    expect_no_match(
        &rule,
        "class A:\n    @classmethod\n    def get_config(cls):\n        pass\n",
        "get_config",
    );
    expect_no_match(
        &rule,
        "class A:\n    @classmethod\n    def __get_pydantic_core_schema__(cls):\n        pass\n",
        "pydantic schema",
    );
}

#[test]
fn classmethod_builder_matches_with_extra_decorator() {
    // Function is still a classmethod even with other decorators above/below
    let rule = load_rule("classmethod-builder-naming");
    expect_match(
        &rule,
        "class A:\n    @property\n    @classmethod\n    def something(cls):\n        pass\n",
        "classmethod + property",
    );
}

// --------------------------------------------------------------------------
// staticmethod-private-only (sculptor: non_private_staticmethods)
// --------------------------------------------------------------------------
#[test]
fn staticmethod_private_matches() {
    let rule = load_rule("staticmethod-private-only");
    expect_match(
        &rule,
        "class A:\n    @staticmethod\n    def something():\n        pass\n",
        "public",
    );
    expect_match(
        &rule,
        "class A:\n    @staticmethod\n    def doit():\n        pass\n",
        "doit",
    );
}

#[test]
fn staticmethod_private_non_matches() {
    let rule = load_rule("staticmethod-private-only");
    expect_no_match(
        &rule,
        "class A:\n    @staticmethod\n    def _something():\n        pass\n",
        "private",
    );
    expect_no_match(
        &rule,
        "class A:\n    @staticmethod\n    def _blah():\n        pass\n",
        "_blah",
    );
}

// --------------------------------------------------------------------------
// attrs-decorator (sculptor: attrs)
// --------------------------------------------------------------------------
#[test]
fn attrs_matches() {
    let rule = load_rule("attrs-decorator");
    expect_match(&rule, "@attr.s()\nclass A:\n    pass\n", "empty");
    expect_match(
        &rule,
        "@attr.s(auto_attribs=True, hash=True, collect_by_mro=True)\nclass A:\n    pass\n",
        "with hash flag",
    );
}

#[test]
fn attrs_non_matches() {
    let rule = load_rule("attrs-decorator");
    expect_no_match(
        &rule,
        "@attr.s(auto_attribs=True, frozen=True)\nclass A:\n    pass\n",
        "frozen",
    );
    expect_no_match(
        &rule,
        "@attr.s(auto_attribs=True)\nclass A:\n    pass\n",
        "auto only",
    );
    expect_no_match(
        &rule,
        "@attr.s(auto_exc=True, auto_attribs=True)\nclass A:\n    pass\n",
        "auto_exc",
    );
    expect_no_match(
        &rule,
        "@attr.s(auto_exc=True, auto_attribs=True, repr=False)\nclass A:\n    pass\n",
        "auto_exc no_repr",
    );
    expect_no_match(
        &rule,
        "@attr.s(auto_attribs=True, repr=False)\nclass A:\n    pass\n",
        "auto_attribs and repr",
    );
    expect_no_match(
        &rule,
        "@attr.s(auto_attribs=True, frozen=True, kw_only=True, repr=False)\nclass A:\n    pass\n",
        "all options",
    );
    expect_no_match(
        &rule,
        "@attr.s(auto_attribs=True, kw_only=True)\nclass A:\n    pass\n",
        "kw_only",
    );
}

// --------------------------------------------------------------------------
// no-mutable-attr-in-frozen-dataclass (sculptor: mutable_attr_in_frozen_dataclass)
// --------------------------------------------------------------------------
#[test]
fn mutable_attr_frozen_matches() {
    let rule = load_rule("no-mutable-attr-in-frozen-dataclass");
    expect_match(
        &rule,
        "@attr.s(auto_attribs=True, frozen=True)\nclass A:\n    a: Dict[str, int]\n",
        "Dict",
    );
    expect_match(
        &rule,
        "@attr.s(auto_attribs=True, frozen=True)\nclass A:\n    a: Set[str]\n",
        "Set",
    );
    expect_match(
        &rule,
        "@attr.s(auto_attribs=True, frozen=True)\nclass A:\n    a: List[str]\n",
        "List",
    );
    expect_match(
        &rule,
        "@attr.s(auto_attribs=True, frozen=True)\nclass A:\n    a: str\n    b: Dict[str, int]\n",
        "after other field",
    );
}

#[test]
fn mutable_attr_frozen_non_matches() {
    let rule = load_rule("no-mutable-attr-in-frozen-dataclass");
    expect_no_match(
        &rule,
        "@attr.s(auto_attribs=True, frozen=True)\nclass A:\n    a: str\n    b: Mapping[str, int]\n",
        "Mapping ok",
    );
    expect_no_match(
        &rule,
        "@attr.s(auto_attribs=True, frozen=True)\nclass A:\n    def thing(x: Dict[str, int]) -> None:\n        pass\n",
        "function param not field",
    );
    expect_no_match(
        &rule,
        "@attr.s(auto_attribs=True)\nclass A:\n    a: Dict[str, int]\n",
        "not frozen ok",
    );
}

// --------------------------------------------------------------------------
// no-inline-functions (sculptor: inline_functions)
// --------------------------------------------------------------------------
// Sculptor's regex relies on indentation + "first arg is not cls/self" to
// approximate "inline function." That heuristic misses several real shapes:
//   - `async def` (the regex starts with `def`, not `(?:async\s+)?def`)
//   - decorator-`wraps` wrappers whose first arg is `self` (because the
//     wrapped function is a method) — sculptor wrongly skips them
//   - inline functions taking `self` as a parameter name (e.g. monkey-patch
//     targets) — sculptor wrongly skips them
// And gives false positives on `def NAME(` appearing inside docstrings.
//
// Our query captures every `function_definition` and uses the
// `nested_in_function_definition` post-filter to keep only those whose AST
// ancestor chain hits a `function_definition` before any `class_definition`.
// This handles direct nested functions, decorated-wrapper nested functions,
// and functions nested inside `if`/`with`/`for`/`try` blocks of an enclosing
// function, while correctly NOT flagging methods of nested classes.
#[test]
fn no_inline_functions_direct_nested_matches() {
    let rule = load_rule_with_python_tests("no-inline-functions");
    expect_match(
        &rule,
        "def outer():\n    def inner():\n        pass\n",
        "direct nested function",
    );
    expect_match(
        &rule,
        "def outer(x):\n    def helper(y):\n        return y + 1\n    return helper(x)\n",
        "nested with args",
    );
}

#[test]
fn no_inline_functions_decorated_nested_matches() {
    // Previously-missed shape: a nested function wrapped by a decorator is
    // wrapped in a `decorated_definition` node, so the old query
    // `(function_definition body: (block (function_definition) @v))` did not
    // match it. The new post-filter approach walks up the parent chain and
    // catches the inner function regardless of intermediate wrappers.
    let rule = load_rule_with_python_tests("no-inline-functions");
    expect_match(
        &rule,
        "import functools\n\
         def sync(func):\n    \
             @functools.wraps(func)\n    \
             def wrapper(*args, **kwargs):\n        \
                 return func(*args, **kwargs)\n    \
             return wrapper\n",
        "@functools.wraps decorated wrapper",
    );
    expect_match(
        &rule,
        "def outer(func):\n    \
             @wraps(func)\n    \
             def wrapper(self, x):\n        \
                 return func(self, x)\n    \
             return wrapper\n",
        "decorated wrapper whose first arg is self",
    );
}

#[test]
fn no_inline_functions_nested_in_block_matches() {
    // Previously-missed shape: a function defined inside an `if`/`with`/`for`
    // /`try` block within an enclosing function is not a direct child of the
    // enclosing function's body block, so the old narrow query missed it.
    let rule = load_rule_with_python_tests("no-inline-functions");
    expect_match(
        &rule,
        "def outer():\n    \
             if cond:\n        \
                 def inner():\n            \
                     pass\n",
        "nested inside if",
    );
    expect_match(
        &rule,
        "def outer():\n    \
             with ctx:\n        \
                 def inner():\n            \
                     pass\n",
        "nested inside with",
    );
    expect_match(
        &rule,
        "def outer():\n    \
             for x in items:\n        \
                 def inner():\n            \
                     pass\n",
        "nested inside for",
    );
    expect_match(
        &rule,
        "def outer():\n    \
             try:\n        \
                 def inner():\n            \
                     pass\n    \
             except Exception:\n        \
                 pass\n",
        "nested inside try",
    );
}

#[test]
fn no_inline_functions_async_def_matches() {
    let rule = load_rule_with_python_tests("no-inline-functions");
    expect_match(
        &rule,
        "async def outer():\n    \
             try:\n        \
                 async def read():\n            \
                     pass\n        \
                 async def write():\n            \
                     pass\n    \
             except Exception:\n        \
                 pass\n",
        "async def inline",
    );
}

#[test]
fn no_inline_functions_deeply_nested_matches() {
    let rule = load_rule_with_python_tests("no-inline-functions");
    expect_match(
        &rule,
        "def outer():\n    \
             def middle():\n        \
                 def inner():\n            \
                     pass\n",
        "three-deep nesting (each inner counted)",
    );
}

#[test]
fn no_inline_functions_top_level_does_not_match() {
    let rule = load_rule_with_python_tests("no-inline-functions");
    expect_no_match(
        &rule,
        "def foo():\n    pass\ndef bar():\n    pass\n",
        "two top-level functions",
    );
}

#[test]
fn no_inline_functions_class_method_does_not_match() {
    let rule = load_rule_with_python_tests("no-inline-functions");
    expect_no_match(
        &rule,
        "class Foo:\n    \
             def bar(self):\n        \
                 pass\n",
        "regular method",
    );
    expect_no_match(
        &rule,
        "class Foo:\n    \
             @staticmethod\n    \
             def bar():\n        \
                 pass\n",
        "staticmethod",
    );
    expect_no_match(
        &rule,
        "class Foo:\n    \
             @classmethod\n    \
             def bar(cls):\n        \
                 pass\n",
        "classmethod",
    );
}

#[test]
fn no_inline_functions_method_of_nested_class_does_not_match() {
    // Methods of a class defined inside a function should NOT be flagged
    // as inline functions — the walk stops at the first `class_definition`
    // ancestor. (The nested class itself is a separate concern.)
    let rule = load_rule_with_python_tests("no-inline-functions");
    expect_no_match(
        &rule,
        "def outer():\n    \
             class Inner:\n        \
                 def method(self):\n            \
                     pass\n",
        "method of nested class",
    );
}

// Additional edge-case validation
#[test]
fn no_bare_exit_self_dot_exit_does_not_match() {
    let rule = load_rule("no-bare-exit");
    expect_no_match(&rule, "self.exit(0)\n", "method exit");
    expect_no_match(&rule, "os.exit(1)\n", "module attribute exit");
}

#[test]
fn no_bare_exit_no_args_does_not_match() {
    // sculptor regex requires \d+ — exit() with no args doesn't match
    let rule = load_rule("no-bare-exit");
    expect_no_match(&rule, "exit()\n", "no args");
}

// --------------------------------------------------------------------------
// no-underscore-imports (sculptor: import_underscore)
// --------------------------------------------------------------------------
// Sculptor's regex is `^(from [\w.]+ )?import __?\w+`, which matches when the
// IMPORTED name (the token right after `import `) starts with one or two
// underscores. The module path in `from MODULE import NAME` is unconstrained,
// so `from _module import foo` is intentionally NOT flagged.
//
// Our previous query matched any `(dotted_name (identifier))` child of an
// `import_from_statement`, which incorrectly fired on the module path too
// (e.g. `from _typeshed import OpenBinaryMode` and
// `from cattrs._compat import is_generic` both falsely matched). Pinning the
// query to the `name:` field of `import_from_statement`/`import_statement`
// fixes the overcount.

#[test]
fn no_underscore_imports_from_import_underscore_name_matches() {
    // Sculptor match_examples: `from thing import _thing`, `from thing.thing import _thing`.
    let rule = load_rule_with_python_tests("no-underscore-imports");
    expect_match(
        &rule,
        "from thing import _thing\n",
        "from MODULE import _NAME",
    );
    expect_match(
        &rule,
        "from thing import __thing\n",
        "from MODULE import __NAME (dunder)",
    );
    expect_match(
        &rule,
        "from thing.thing import _thing\n",
        "from MODULE.SUB import _NAME",
    );
    expect_match(
        &rule,
        "from sculptor.testing.server_utils import _start_server_process_and_validate_readiness\n",
        "real sculptor violation 1",
    );
    expect_match(
        &rule,
        "from imbue_core.nested_evolver import _Evolver\n",
        "real sculptor violation 2",
    );
}

#[test]
fn no_underscore_imports_aliased_underscore_name_matches() {
    // Sculptor's regex matches `from x import _y as z` because the substring
    // `import _y` is present. Tree-sitter wraps this in an `aliased_import`
    // node, so we need a dedicated pattern.
    let rule = load_rule_with_python_tests("no-underscore-imports");
    expect_match(
        &rule,
        "from foo import _bar as baz\n",
        "from MODULE import _NAME as ALIAS",
    );
    expect_match(
        &rule,
        "from foo import _bar as _baz\n",
        "from MODULE import _NAME as _ALIAS",
    );
}

#[test]
fn no_underscore_imports_bare_import_underscore_matches() {
    // Sculptor match_examples: `import _thing`, `import __thing`.
    // Our previous query missed these because it only handled
    // `import_from_statement`, not the distinct `import_statement` node.
    let rule = load_rule_with_python_tests("no-underscore-imports");
    expect_match(&rule, "import _thing\n", "import _NAME");
    expect_match(&rule, "import __thing\n", "import __NAME (dunder)");
    expect_match(&rule, "import _thing as t\n", "import _NAME as ALIAS");
}

#[test]
fn no_underscore_imports_from_private_module_does_not_match() {
    // Sculptor's `^(from [\w.]+ )?import __?\w+` does NOT match
    // `from _module import foo` because the imported name is `foo`, not `_*`.
    // The +62 overcount in code-eu8 was driven by our prior query catching
    // these cases (e.g. `from _typeshed import OpenBinaryMode`,
    // `from cattrs._compat import is_generic`).
    let rule = load_rule_with_python_tests("no-underscore-imports");
    expect_no_match(
        &rule,
        "from _typeshed import OpenBinaryMode\n",
        "from _MODULE import NAME (top-level)",
    );
    expect_no_match(
        &rule,
        "from cattrs._compat import is_generic\n",
        "from MODULE._SUB import NAME (submodule path)",
    );
    expect_no_match(
        &rule,
        "from sculpt.commands._follow_helpers import follow_and_stream_messages\n",
        "from PKG.SUB._PRIVATE import name",
    );
    expect_no_match(
        &rule,
        "from _pytest.junitxml import xml_key\n",
        "from _PKG.SUB import name",
    );
    expect_no_match(
        &rule,
        "from loguru._file_sink import FileSink\n",
        "from PKG._SUB import Name",
    );
}

#[test]
fn no_underscore_imports_regular_imports_do_not_match() {
    // Sculptor non_match_examples: `import stuff`, `from stuff import thing`.
    let rule = load_rule_with_python_tests("no-underscore-imports");
    expect_no_match(&rule, "import stuff\n", "import NAME");
    expect_no_match(
        &rule,
        "from stuff import thing\n",
        "from MODULE import NAME",
    );
    expect_no_match(
        &rule,
        "from stuff import thing as other\n",
        "from MODULE import NAME as ALIAS",
    );
    expect_no_match(&rule, "import stuff as s\n", "import NAME as ALIAS");
}

#[test]
fn no_underscore_imports_dotted_import_first_component_only() {
    // Sculptor's regex requires the token right after `import ` to start with
    // `_`, so `import foo._bar` does not match but `import _foo.bar` does.
    // The `.` anchor on `(dotted_name . (identifier))` pins our predicate to
    // the first identifier in the dotted path.
    let rule = load_rule_with_python_tests("no-underscore-imports");
    expect_no_match(
        &rule,
        "import foo._bar\n",
        "import PUBLIC.PRIVATE not flagged",
    );
    expect_match(&rule, "import _foo.bar\n", "import _PRIVATE.PUBLIC flagged");
}

#[test]
fn no_underscore_imports_multi_name_counts_per_offender() {
    // `from foo import _bar, _baz` has two underscore-prefixed imports. Our
    // tree-sitter query iterates over each `name:` child and emits one
    // violation per offender. Sculptor's line-based regex only fires once per
    // line and additionally misses this case entirely if the first name is
    // not underscore-prefixed (e.g. `from foo import bar, _baz`), so we are
    // strictly stricter here. This is intentional: each offending name is an
    // independent fix.
    let rule = load_rule_with_python_tests("no-underscore-imports");
    assert_eq!(
        matches(&rule, "from foo import _bar, _baz\n"),
        2,
        "two underscore-prefixed imports should produce two violations",
    );
    assert_eq!(
        matches(&rule, "from foo import bar, _baz\n"),
        1,
        "one underscore-prefixed import among several should produce one violation",
    );
    assert_eq!(
        matches(&rule, "from foo import bar, baz\n"),
        0,
        "no underscore-prefixed imports should produce no violations",
    );
}

#[test]
fn no_underscore_imports_relative_imports_handled() {
    // `from . import _bar` and `from .foo import _bar` are valid Python.
    // Sculptor's regex `from [\w.]+` requires `\w` after `from `, so it does
    // NOT match `from . import ...` (the dot is not a word char). It does
    // match `from .foo import _bar` only with the `from MODULE` prefix
    // omitted (the alternate branch `(from [\w.]+ )?` is optional, and
    // `^...import __?\w+` is the actual match — but `from .foo ` makes the
    // optional group fail; the regex would still match because `^import` is
    // not at line start. Let's not over-interpret. The structurally correct
    // behavior is: the imported name (not the module path) is what matters.
    // `from .foo import _bar` should be flagged because `_bar` is the
    // imported name.
    let rule = load_rule_with_python_tests("no-underscore-imports");
    expect_match(&rule, "from . import _bar\n", "from . import _NAME");
    expect_match(
        &rule,
        "from .foo import _bar\n",
        "from .MODULE import _NAME",
    );
    expect_no_match(
        &rule,
        "from . import bar\n",
        "from . import NAME (not underscore)",
    );
}
