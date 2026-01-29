# Test fixture for eval/exec usage rules
# Tests: no-eval-usage, no-exec-usage

# Should trigger no-eval-usage (3 cases)
def use_eval_simple():
    result = eval("1 + 1")  # Line 6 - should be detected
    return result

def use_eval_with_expression():
    expression = "x * 2"
    value = eval(expression)  # Line 11 - should be detected
    return value

def use_eval_nested():
    data = eval(input("Enter expression: "))  # Line 15 - should be detected
    return data

# Should trigger no-exec-usage (3 cases)
def use_exec_simple():
    exec("print('hello')")  # Line 20 - should be detected

def use_exec_with_code():
    code = "x = 42"
    exec(code)  # Line 24 - should be detected

def use_exec_multiline():
    exec("""
x = 1
y = 2
print(x + y)
""")  # Line 27 - should be detected

# False positives - strings should NOT trigger
def string_literals():
    message = "You should not use eval() in production"
    warning = "exec() can be dangerous"
    code = """
    # Don't use eval
    result = eval(expression)
    """
    return message, warning, code

# False positives - variable names should NOT trigger
def variable_names():
    eval_result = "this is fine"
    exec_mode = "also fine"
    evaluate = lambda x: x * 2
    execute = lambda: None
    return eval_result, exec_mode, evaluate, execute

# Valid alternatives should NOT trigger
def valid_alternatives():
    # Use ast.literal_eval instead
    import ast
    result = ast.literal_eval("[1, 2, 3]")
    return result
