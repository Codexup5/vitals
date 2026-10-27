const WRAPPERS = new Set(['TSAsExpression', 'TSSatisfiesExpression']);

const isCustomProperty = (property) =>
  property.type === 'Property' &&
  property.key.type === 'Literal' &&
  typeof property.key.value === 'string' &&
  property.key.value.startsWith('--');

const unwrap = (expression) => {
  let current = expression;
  while (WRAPPERS.has(current.type)) current = current.expression;
  return current;
};

const isUndefined = (expression) => expression.type === 'Identifier' && expression.name === 'undefined';

const findConstInit = (scope, name) => {
  for (let current = scope; current; current = current.upper) {
    const variable = current.set.get(name);
    if (!variable) continue;
    const [definition] = variable.defs;
    if (variable.defs.length !== 1 || definition?.type !== 'Variable') return null;
    const declaration = definition.parent;
    if (declaration?.type !== 'VariableDeclaration' || declaration.kind !== 'const') return null;
    return definition.node.id.type === 'Identifier' ? definition.node.init : null;
  }
  return null;
};

export default {
  meta: {
    type: 'problem',
    docs: { description: 'Static styles live in CSS modules; style only passes CSS custom properties' },
    messages: {
      inline:
        'Inline style. Static styles live in `*.module.scss` next to the component; `style` carries only CSS variables (`--x`).',
    },
    schema: [],
  },
  create(context) {
    const isAccepted = (expression, scope) => {
      const value = unwrap(expression);
      if (value.type === 'ObjectExpression') return value.properties.every(isCustomProperty);
      if (value.type === 'ConditionalExpression') {
        return [value.consequent, value.alternate].every(
          (branch) => isUndefined(unwrap(branch)) || isAccepted(branch, scope),
        );
      }
      if (value.type === 'Identifier') {
        const init = findConstInit(scope, value.name);
        return init ? unwrap(init).type === 'ObjectExpression' && isAccepted(init, scope) : false;
      }
      return false;
    };

    return {
      JSXAttribute(node) {
        if (node.name.type !== 'JSXIdentifier' || node.name.name !== 'style') return;
        const expression = node.value?.type === 'JSXExpressionContainer' ? node.value.expression : null;
        if (expression && isAccepted(expression, context.sourceCode.getScope(node))) return;
        context.report({ node, messageId: 'inline' });
      },
    };
  },
};
