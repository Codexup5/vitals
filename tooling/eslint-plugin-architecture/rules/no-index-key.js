const ITERATORS = new Set(['map', 'flatMap']);

const isIteratorCallback = (node) => {
  const call = node.parent;
  if (call.type !== 'CallExpression' || call.arguments[0] !== node) return false;
  const { callee } = call;
  return (
    callee.type === 'MemberExpression' &&
    !callee.computed &&
    callee.property.type === 'Identifier' &&
    ITERATORS.has(callee.property.name)
  );
};

const usesName = (node, name) => {
  switch (node.type) {
    case 'Identifier':
      return node.name === name;
    case 'TemplateLiteral':
      return node.expressions.some((expression) => usesName(expression, name));
    case 'CallExpression':
      return usesName(node.callee, name) || node.arguments.some((argument) => usesName(argument, name));
    case 'MemberExpression':
      return usesName(node.object, name);
    case 'BinaryExpression':
      return usesName(node.left, name) || usesName(node.right, name);
    default:
      return false;
  }
};

export default {
  meta: {
    type: 'problem',
    docs: { description: 'A list item is not keyed by its index in map or flatMap' },
    messages: {
      index: 'Key a list item by a stable id of the item, not by its index: the index breaks state on reorder.',
    },
    schema: [],
  },
  create(context) {
    const callbacks = [];

    const enter = (node) => {
      if (!isIteratorCallback(node)) return;
      const index = node.params[1];
      callbacks.push(index?.type === 'Identifier' ? index.name : null);
    };
    const exit = (node) => {
      if (isIteratorCallback(node)) callbacks.pop();
    };

    return {
      FunctionExpression: enter,
      ArrowFunctionExpression: enter,
      'FunctionExpression:exit': exit,
      'ArrowFunctionExpression:exit': exit,
      JSXAttribute(node) {
        if (node.name.type !== 'JSXIdentifier' || node.name.name !== 'key') return;
        if (node.value?.type !== 'JSXExpressionContainer') return;
        const index = callbacks.at(-1);
        if (index && usesName(node.value.expression, index)) context.report({ node, messageId: 'index' });
      },
    };
  },
};
