# Test fixture for comment rules
# Tests: comment detection rules

# Five cases that should be caught for T0D0 detection
# TODO: Implement this feature
def function_with_todo():
    # TODO: Add validation
    pass

# todo: lowercase also matches
def lowercase_todo():
    pass

# This is a TODO comment
def todo_in_middle():
    pass

def another_function():
    # TODO(alice): Refactor this
    pass

# Five cases that should be caught for F1XME detection
# FIXME: This is broken
def function_with_fixme():
    # FIXME: Memory leak here
    pass

# fixme: lowercase also matches
def lowercase_fixme():
    pass

# This is a FIXME comment
def fixme_in_middle():
    pass

def final_function():
    # FIXME(bob): Fix before release
    pass

# False positives - strings should NOT trigger (AST rules only match comments)
def string_literals():
    message = "TODO: This is in a string"
    warning = "FIXME: This is also in a string"
    code = """
    # TODO: This is in a multiline string
    # FIXME: Also in a multiline string
    """
    return message, warning, code

# Docstrings should NOT trigger (AST rules only match comments, not docstrings)
def function_with_docstring():
    """
    TODO: This is in a docstring and should not trigger
    FIXME: This is also in a docstring and should not trigger
    """
    pass

class ClassWithDocstring:
    """
    TODO: Class docstring
    FIXME: Another docstring
    """
    pass

# Valid comments should NOT trigger
# This is a regular comment
# Note: This is important
# HACK: This is a different marker
# XXX: Also different
def clean_function():
    # Regular comment
    pass
