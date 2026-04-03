module.exports = grammar({
  name: 'auwgent',

  extras: $ => [
    /\s/,
    $.comment,
  ],

  word: $ => $.identifier,

  conflicts: $ => [
    [$.expr, $.property_name],
    [$.condition, $.expr],
    [$.condition, $.grouped_expr],
    [$.index_access, $.member_access],
    [$.call_expr, $.identifier],
    [$.index_access],
    [$.property_declaration],
    [$.block, $.prompt_block_internal],
    [$.prompt_statement, $.statement],
    [$.prompt_statement, $.block],
  ],

  rules: {
    source_file: $ => repeat($._definition),

    _definition: $ => choice(
      $.import_declaration,
      $.export_declaration,
      $.agent_declaration,
      $.helper_declaration,
      $.type_declaration,
      $.component_declaration,
      $.named_prompt_declaration,
      $.model_definition,
      $.intent_declaration
    ),

    // ── Top-level Declarations ───────────────────────────────────────────

    import_declaration: $ => seq(
      'import',
      choice(
        $.named_imports,
        seq('*', 'as', $.identifier)
      ),
      'from',
      $.string_literal
    ),

    named_imports: $ => seq(
      '{',
      commaSep($.identifier),
      '}'
    ),

    export_declaration: $ => seq(
      'export',
      choice(
        $.helper_declaration,
        $.type_declaration,
        $.component_declaration,
        $.named_prompt_declaration,
        $.model_definition,
        $.intent_declaration
      )
    ),

    agent_declaration: $ => seq(
      'agent',
      $.identifier,
      $.agent_body
    ),

    agent_body: $ => seq(
      '{',
      repeat($.agent_config),
      '}'
    ),

    helper_declaration: $ => seq(
      'helper',
      $.identifier,
      $.helper_body
    ),

    helper_body: $ => seq(
      '{',
      'description', ':', $.string_literal,
      repeat($.agent_config),
      '}'
    ),

    type_declaration: $ => seq(
      'type',
      $.identifier,
      $.type_body
    ),

    component_declaration: $ => seq(
      'component',
      $.identifier,
      $.component_body
    ),

    component_body: $ => seq(
      '{',
      repeat($.component_field),
      '}'
    ),

    component_field: $ => choice(
      $.property_declaration,
      $.component_action_field,
      $.component_children_field
    ),

    component_action_field: $ => prec(2, seq(
      'action',
      ':',
      '{',
      repeat($.component_action_binding),
      '}'
    )),

    component_action_binding: $ => seq(
      $.identifier,
      ':',
      $.identifier,
      repeat(seq('|', $.identifier))
    ),

    component_children_field: $ => prec(2, seq(
      'children',
      ':',
      choice(
        'all',
        seq($.identifier, repeat(seq('|', $.identifier)))
      )
    )),

    type_body: $ => seq(
      '{',
      repeat($.property_declaration),
      '}'
    ),

    named_prompt_declaration: $ => seq(
      'prompt',
      $.identifier,
      optional($.parameters),
      $.prompt_body_block
    ),

    prompt_body_block: $ => seq(
        '{',
        repeat($.prompt_statement),
        '}'
    ),

    model_definition: $ => seq(
      'model',
      $.identifier,
      choice(
        seq('=', $.model_provider),
        seq('{', 'provider', ':', $.model_provider, '}')
      )
    ),

    intent_declaration: $ => seq(
      'intent',
      $.intent_body
    ),

    intent_body: $ => seq(
      $.identifier,
      '{',
      optional(seq('description', ':', $.string_literal)),
      optional(seq('fields', '{', repeat($.property_declaration), '}')),
      '}'
    ),

    // ── Agent Configurations ─────────────────────────────────────────────

    agent_config: $ => choice(
        $.input_config,
        $.output_config,
        $.context_config,
        $.tools_config,
        $.tool_single_config,
        $.helpers_config,
        $.model_config_block,
        $.lifecycle_config,
        $.test_config,
        $.workflow_config,
        $.intent_config
    ),

    input_config: $ => seq(
        'input',
        choice(
            seq(':', $.type_expr),
            seq('{', repeat($.property_declaration), '}')
        )
    ),

    output_config: $ => seq(
        'output',
        choice(
            seq(':', $.output_direct_shape),
            seq('{', repeat($.output_property_declaration), '}')
        )
    ),

    output_direct_shape: $ => choice(
        seq($.identifier, repeat1(seq('|', $.identifier))), // Union of types
        seq($.type_expr, optional(seq('@desc', $.string_literal)))
    ),

    output_property_declaration: $ => seq(
        $.property_declaration,
        optional(seq('@desc', $.string_literal))
    ),

    context_config: $ => seq(
        'context',
        '{',
        repeat($.property_declaration),
        '}'
    ),

    tools_config: $ => seq(
        'tools',
        '{',
        repeat($.tool_function),
        '}'
    ),

    tool_single_config: $ => seq(
        'tool',
        $.tool_function
    ),

    helpers_config: $ => seq(
        'helpers',
        '{',
        repeat($.helper_ref),
        '}'
    ),

    helper_ref: $ => seq(
        $.identifier,
        optional(seq(
            'with',
            choice(
                seq('all', 'tools'),
                seq('tools', '{', commaSep($.identifier), '}')
            )
        )),
        optional(seq('handoff', 'user')),
        optional(seq('then', 'continue'))
    ),

    model_config_block: $ => choice(
        seq('default', 'config', '{', repeat($._model_setting), '}'),
        seq('config', $.identifier, '{', repeat($._model_setting), '}')
    ),

    _model_setting: $ => choice(
        seq('model', ':', $.model_provider_ref),
        seq('embedding', ':', $.model_provider_ref),
        seq('prompt', choice(
            seq(':', $.expr),
            seq('{', repeat($.prompt_statement), '}')
        ))
    ),

    model_provider_ref: $ => choice(
        $.model_provider,
        $.identifier
    ),

    model_provider: $ => choice(
        seq('gemini', '(', $.string_literal, optional(seq(',', $.object_literal)), ')'),
        seq('openai', '(', $.string_literal, optional(seq(',', $.object_literal)), ')'),
        seq('custom', '(', $.string_literal, ',', $.string_literal, ',', $.string_literal, optional(seq(',', $.object_literal)), ')')
    ),

    lifecycle_config: $ => seq(
        'use', 'lifecycle',
        '{',
        repeat(choice(
            seq('maxTokens', ':', $.number_literal),
            seq('maxMessages', ':', $.number_literal)
        )),
        '}'
    ),

    test_config: $ => seq(
        'test',
        $.string_literal,
        optional(seq('config', ':', $.identifier)),
        $.block // Test body is treated as a block for now
    ),

    workflow_config: $ => seq(
        'workflow',
        $.identifier,
        $.parameters,
        optional(seq(':', $.type_expr)),
        $.workflow_body
    ),

    workflow_body: $ => seq(
        '{',
        optional(seq('description', ':', $.string_literal)),
        repeat(choice(
            seq('tool', $.tool_function),
            seq('tools', '{', repeat($.tool_function), '}')
        )),
        repeat($.statement),
        '}'
    ),

    intent_config: $ => seq(
        'intent', ':', $.intent_expr
    ),

    intent_expr: $ => prec.left(seq(
        $._intent_atom,
        repeat(seq('+', $._intent_atom))
    )),

    _intent_atom: $ => choice(
        $.identifier,
        seq('{', repeat1($.intent_body), '}')
    ),

    // ── Functions & Types ────────────────────────────────────────────────

    tool_function: $ => seq(
        $.identifier,
        $.parameters,
        optional(seq(':', $.type_expr)),
        repeat(seq('@desc', $.string_literal))
    ),

    property_declaration: $ => seq(
        $.property_name,
        optional('?'),
        ':',
        $.type_expr,
        optional(seq('@desc', $.string_literal))
    ),

    parameters: $ => seq(
        '(',
        commaSep($.parameter),
        ')'
    ),

    parameter: $ => seq(
        $.identifier,
        optional('?'),
        ':',
        $.type_expr
    ),

    type_expr: $ => choice(
        $.primitive_type,
        $.identifier,
        $.object_type,
        $.array_type,
        $.union_type
    ),

    primitive_type: $ => choice(
        'string',
        'number',
        'boolean',
        'Text'
    ),

    object_type: $ => seq(
        '{',
        repeat($.property_declaration),
        '}'
    ),

    array_type: $ => seq(
        $.type_expr,
        '[]'
    ),

    union_type: $ => seq(
        $.string_literal,
        repeat1(seq('|', $.string_literal))
    ),

    // ── Statements ───────────────────────────────────────────────────────

    statement: $ => choice(
        $.let_statement,
        $.return_statement,
        $.if_statement,
        $.transfer_statement,
        $.parallel_statement,
        $.assignment_statement,
        $.expr_statement
    ),

    let_statement: $ => seq(
        'let',
        $.identifier,
        optional(seq(':', $.type_expr)),
        '=',
        $.expr
    ),

    return_statement: $ => seq(
        'return',
        $.expr
    ),

    if_statement: $ => seq(
        'if',
        seq('(', $.condition, ')'),
        $.block,
        optional(seq('else', $.block))
    ),

    block: $ => seq(
        '{',
        repeat($.statement),
        '}'
    ),

    transfer_statement: $ => seq(
        'transfer',
        'to',
        'hlp', '.', $.identifier,
        '(', commaSep($.expr), ')',
        optional(seq('then', 'continue'))
    ),

    parallel_statement: $ => seq(
        'parallel',
        $.block
    ),

    assignment_statement: $ => seq(
        $.identifier,
        '=',
        $.expr
    ),

    expr_statement: $ => $.expr,

    prompt_statement: $ => choice(
        $.example_block,
        $.prompt_if,
        $.statement
    ),

    example_block: $ => seq(
        'example',
        '{',
        repeat($.example_message),
        '}'
    ),

    example_message: $ => seq(
        choice('user', 'assistant'),
        ':',
        $.string_literal
    ),

    prompt_if: $ => seq(
        'if',
        seq('(', $.condition, ')'),
        $.prompt_block_internal,
        optional(seq('else', $.prompt_block_internal))
    ),

    prompt_block_internal: $ => seq(
        '{',
        repeat($.prompt_statement),
        '}'
    ),

    // ── Expressions ──────────────────────────────────────────────────────

    expr: $ => choice(
        $.binary_op,
        $.member_access,
        $.index_access,
        $.call_expr,
        $.identifier,
        $.string_literal,
        $.number_literal,
        $.boolean_literal,
        $.object_literal,
        $.inline_prompt,
        $.context_ref,
        $.helper_call,
        $.grouped_expr
    ),

    binary_op: $ => choice(
        prec.left(2, seq($.expr, choice('*', '/'), $.expr)),
        prec.left(1, seq($.expr, choice('+', '-'), $.expr))
    ),

    condition: $ => choice(
        prec.left(2, seq($.condition, '&&', $.condition)),
        prec.left(1, seq($.condition, '||', $.condition)),
        $.comparison,
        seq('(', $.condition, ')'),
        $.expr // Boolean expr
    ),

    comparison: $ => seq(
        $.expr,
        choice('==', '!=', '>=', '<=', '>', '<'),
        $.expr
    ),

    member_access: $ => prec(3, seq(
        $.expr,
        '.',
        $.identifier
    )),

    index_access: $ => prec(3, seq(
        $.expr,
        '[', $.expr, ']',
        repeat(seq('.', $.identifier))
    )),

    call_expr: $ => prec(4, seq(
        $.identifier,
        '(',
        commaSep($.expr),
        ')'
    )),

    object_literal: $ => seq(
        '{',
        commaSep($.property_assignment),
        '}'
    ),

    property_assignment: $ => seq(
        $.property_name,
        optional(seq(':', $.expr)) // Support shorthand { name }
    ),

    inline_prompt: $ => seq(
        '{',
        repeat1($.expr), // Simplified: in practice parts are exprs
        '}'
    ),

    context_ref: $ => seq(
        'ctx',
        '.',
        $.property_name
    ),

    helper_call: $ => seq(
        'hlp',
        '.',
        $.identifier,
        '(',
        commaSep($.expr),
        ')'
    ),

    grouped_expr: $ => seq(
        '(',
        $.expr,
        ')'
    ),

    // ── Shared Primitives ────────────────────────────────────────────────

    property_name: $ => choice(
        $.identifier,
        'model', 'prompt', 'config', 'input', 'output', 'context',
        'description', 'error', 'provider', 'maxTokens', 'maxMessages'
    ),

    identifier: $ => /[_a-zA-Z][_a-zA-Z0-9]*/,

    string_literal: $ => choice(
        $._double_string,
        $._single_string,
        $._multiline_string
    ),

    _double_string: $ => /"([^"\\]|\\.)*"/,
    _single_string: $ => /'([^'\\]|\\.)*'/,
    _multiline_string: $ => /"""(?:[^"]|"[^"]|""[^"])*"""/,

    number_literal: $ => /[0-9]+(\.[0-9]+)?/,

    boolean_literal: $ => choice('true', 'false'),

    comment: $ => token(choice(
        seq('//', /[^\n\r]*/),
        seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')
    ))
  }
});

function commaSep(rule) {
  return optional(seq(rule, repeat(seq(',', rule))));
}
function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}
