# Test fixture for exception handling rules
# Tests: no-base-exception, no-broad-exception

# Should trigger no-base-exception (2 cases)
def catch_base_exception_simple():
    try:
        risky_operation()
    except BaseException:  # Line 8 - should be detected
        pass

def catch_base_exception_with_as():
    try:
        another_operation()
    except BaseException as e:  # Line 14 - should be detected
        print(f"Error: {e}")

# Should trigger no-broad-exception (2 cases)
def catch_broad_exception_simple():
    try:
        operation()
    except Exception:  # Line 21 - should be detected
        pass

def catch_broad_exception_with_as():
    try:
        operation()
    except Exception as e:  # Line 27 - should be detected
        print(f"Error: {e}")

# False positives - strings should NOT trigger
def string_literals():
    message = "except BaseException: should not match"
    error_msg = "except Exception: should not match"
    code = """
    try:
        something()
    except BaseException:
        pass
    """
    return message, error_msg, code

# Valid exception handling - specific types should NOT trigger
def valid_exception_handling():
    try:
        operation()
    except ValueError:
        print("value error")
    except KeyError as e:
        print(f"key error: {e}")
    except (TypeError, AttributeError):
        print("type or attribute error")
