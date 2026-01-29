# Test fixture for control flow rules
# Tests: no-while-true, no-global-keyword, no-bare-print

# Should trigger no-while-true (3 cases)
def infinite_loop_simple():
    while True:  # Line 6 - should be detected
        print("forever")
        break

def infinite_loop_with_condition():
    while True:  # Line 11 - should be detected
        if some_condition():
            break

def nested_while_true():
    while some_outer_condition():
        while True:  # Line 17 - should be detected
            if inner_condition():
                break

# Should trigger no-global-keyword (3 cases)
counter = 0

def use_global_simple():
    global counter  # Line 25 - should be detected
    counter += 1

def use_global_multiple():
    global counter, another_var  # Line 29 - should be detected
    counter = 0
    another_var = 0

def nested_global():
    def inner():
        global counter  # Line 35 - should be detected
        counter += 1
    inner()

# Should trigger no-bare-print (4 cases)
def use_print_simple():
    print("hello world")  # Line 41 - should be detected

def use_print_multiple():
    print("first")  # Line 44 - should be detected
    print("second")  # Line 45 - should be detected

def use_print_formatted():
    name = "Alice"
    print(f"Hello {name}")  # Line 49 - should be detected

# False positives - strings should NOT trigger
def string_literals():
    message = "while True: should not match"
    code = "global counter"
    instruction = "print('hello')"
    return message, code, instruction

# Valid alternatives should NOT trigger
def valid_alternatives():
    # Use explicit conditions
    while counter < 10:
        counter += 1

    # Pass state through parameters
    def increment(count):
        return count + 1

    # Use logger instead of print
    import logging
    logger = logging.getLogger(__name__)
    logger.info("This is fine")
    logger.debug("Also fine")

    # Print in return expressions is a function call, not a statement
    return print  # This returns the print function, not calling it
