We are migrating and restructuring our codebase, you have creative freedom, to take reference and reimplement any structures.


&variable refers to variable's address (getting a pointer/reference)

&mut variable refers to variable's mutable address (getting a mutable pointer/reference)

@T refers to a pointer type of T

variable @= expression means copying expression's value at the variable's memory location (pointer storage)

value = @variable means dereferencing the pointer and resolves to its value

'#function() is a macro, that is compile time function call that can be used for metaprogramming'

everything else is like any other programming languages