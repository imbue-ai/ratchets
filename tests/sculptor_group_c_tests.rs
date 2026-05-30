//! Validation tests for Group C sculptor rules ported as tree-sitter AST queries.
//!
//! Each rule covers match_examples and non_match_examples from sculptor's
//! ratchet_rules.py to ensure the tree-sitter form matches the original regex intent.

#![cfg(feature = "lang-python")]

mod sculptor_common;

use sculptor_common::{expect_match, expect_no_match, load_rule};

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
