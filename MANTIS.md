We are migrating and restructuring our codebase, you have creative freedom, to take reference and reimplement any structures.

mut variable refers to mutable variable

&variable refers to variable's address (getting a pointer/reference)

&mut variable refers to variable's mutable address (getting a mutable pointer/reference)

@T refers to a pointer type of T

variable @= expression means copying expression's value at the variable's memory location (pointer storage)

value = @variable means dereferencing the pointer and resolves to its value

'#function() is a macro, that is compile time function call that can be used for metaprogramming'

'#size_of(type)' returns size in bytes of type

'#ref(@type)' takes in pointer and returns reference type &type

'#ptr(&type)' takes in reference and returns pointer @type

'#init(type)' returns a pointer to heap allocated type @type, must be freed using #free(@type)

everything else is like any other programming languages


run python tests/test_runner.py to make sure changes don't break other tests