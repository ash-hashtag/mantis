; Tree-sitter textobject queries for the Mantis grammar.

(function_declaration (block) @function.inside) @function.around

(impl_declaration (block) @class.inside) @class.around
(trait_declaration (block) @class.inside) @class.around

(parameter) @parameter.inside
(parameter) @parameter.around

(comment) @comment.inside
(comment)+ @comment.around
