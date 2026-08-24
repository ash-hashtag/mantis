; Tree-sitter indent queries for the Mantis grammar.

[
  (block)
  (parameter_list)
  (enum_body)
  (struct_body)
  (array_expression)
] @indent

[
  "}"
  ")"
  "]"
] @outdent
