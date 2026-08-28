// A deliberately permissive grammar: Mantis' compiler remains the authority
// for semantic validation, while this grammar keeps editor parsing resilient
// as the language evolves.
module.exports = grammar({
  name: 'mantis',

  extras: $ => [$.comment, /[\s\t\r\n]/],
  word: $ => $.identifier,
  conflicts: $ => [[$.static_declaration, $.expression], [$.expression, $.struct_expression], [$.function_declaration, $.expression]],

  rules: {
    source_file: $ => repeat(choice($.declaration, $.statement, $.expression)),

    declaration: $ => choice(
      $.function_declaration, $.type_declaration, $.static_declaration,
      $.use_declaration, $.import_declaration, $.trait_declaration,
      $.impl_declaration
    ),
    function_declaration: $ => seq(optional($.pub), optional($.async), $.fn, field('name', $.identifier),
      $.parameter_list,
      choice(
        seq($.return_type, optional($.extern), optional($.block), optional(';')),
        seq($.extern, ';'),
        seq($.block, optional(';'))
      )),
    type_declaration: $ => seq($.type, field('name', $.identifier), '=',
      choice($.struct_body, $.enum_body, $.type_expression), optional(';')),
    static_declaration: $ => seq(optional($.static), optional($.const), field('name', $.identifier),
      optional(seq(':', $.type_expression)), '=', $.expression, ';'),
    use_declaration: $ => seq($.use, $.path, optional(';')),
    import_declaration: $ => seq($.import, $.path, optional(';')),
    trait_declaration: $ => seq($.trait, $.identifier, $.block),
    impl_declaration: $ => seq($.impl, repeat1($.type_expression), $.block),

    block: $ => seq('{', repeat(choice($.declaration, $.statement, $.expression)), '}'),
    statement: $ => choice($.let_statement, $.return_statement, $.if_statement, $.loop_statement,
      $.break_statement, $.continue_statement, $.expression_statement),
    let_statement: $ => seq($.let, optional($.mut), $.identifier, optional(seq(':', $.type_expression)), '=', $.expression, ';'),
    return_statement: $ => choice(seq($.return, $.expression, optional(';')), seq($.return, ';')),
    if_statement: $ => seq($.if, $.expression, $.block, repeat(seq($.elif, $.expression, $.block)), optional(seq($.else, $.block))),
    loop_statement: $ => seq($.loop, $.block),
    break_statement: $ => seq($.break, optional($.identifier), ';'),
    continue_statement: $ => seq($.continue, optional($.identifier), ';'),
    expression_statement: $ => seq($.expression, ';'),

    expression: $ => choice($.call_expression, $.field_expression, $.binary_expression,
      $.unary_expression, $.struct_expression, $.array_expression, $.literal, $.path, $.identifier),
    call_expression: $ => prec(10, seq(field('function', choice($.path, $.field_expression, $.identifier)), '(', optional(commaSep($.expression)), ')')),
    field_expression: $ => prec.left(11, seq(field('object', $.expression), '.', field('field', $.identifier))),
    binary_expression: $ => choice(
      prec.left(5, seq($.expression, choice('=', '==', '!=', '<', '>', '<=', '>=', '&&', '||'), $.expression)),
      prec.left(6, seq($.expression, choice('+', '-', '|', '^'), $.expression)),
      prec.left(7, seq($.expression, choice('*', '/', '%', '&'), $.expression))
    ),
    unary_expression: $ => prec(9, seq(choice('!', '-', '&', '@'), $.expression)),
    struct_expression: $ => seq($.identifier, '{', optional(commaSep($.field_initializer)), '}'),
    field_initializer: $ => seq($.identifier, choice(':', '='), $.expression),
    array_expression: $ => seq('[', optional(commaSep($.expression)), ']'),

    parameter_list: $ => seq('(', optional(commaSep($.parameter)), ')'),
    parameter: $ => seq(optional($.mut), $.identifier, optional(seq(':', $.type_expression))),
    generic_parameters: $ => seq('[', commaSep($.type_expression), ']'),
    return_type: $ => $.type_expression,
    type_expression: $ => choice($.path, $.identifier),
    generic_type: $ => seq($.path, '[', commaSep($.type_expression), ']'),
    pointer_type: $ => seq('@', optional($.mut), choice($.path, $.identifier)),
    path: $ => prec.left(12, seq($.identifier, repeat1(seq('.', $.identifier)))),
    struct_body: $ => seq($.struct, '{', repeat($.parameter), '}'),
    enum_body: $ => seq($.enum, '{', repeat($.enum_variant), '}'),
    enum_variant: $ => seq($.identifier, optional(seq('(', commaSep($.type_expression), ')')), optional(',')),
    literal: $ => choice($.number, $.float, $.string, $.char, $.boolean),
    number: _ => /[0-9]+/,
    float: _ => /[0-9]+\.[0-9]+/,
    string: _ => /"([^"\\]|\\.)*"/,
    char: _ => /'([^'\\]|\\.)'/,
    boolean: _ => choice('true', 'false'),
    identifier: _ => /[A-Za-z_][A-Za-z0-9_]*/,
    comment: _ => token(choice(seq('//', /.*/), seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'))),

    pub: _ => 'pub', fn: _ => 'fn', let: _ => 'let', mut: _ => 'mut', return: _ => 'return',
    if: _ => 'if', elif: _ => 'elif', else: _ => 'else', loop: _ => 'loop',
    break: _ => 'break', continue: _ => 'continue', type: _ => 'type',
    struct: _ => 'struct', enum: _ => 'enum', trait: _ => 'trait', impl: _ => 'impl',
    extern: _ => 'extern', import: _ => 'import', use: _ => 'use', async: _ => 'async',
    await: _ => 'await', yield: _ => 'yield', where: _ => 'where', static: _ => 'static',
    const: _ => 'const'
  }
});

function commaSep(rule) { return optional(seq(rule, repeat(seq(',', rule)))); }
