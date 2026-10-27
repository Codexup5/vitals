const LAZY_KEYS = new Set(['queryFn', 'mutationFn']);

export default {
  meta: {
    type: 'problem',
    docs: { description: 'queryFn and mutationFn are closures, not direct references' },
    messages: {
      direct:
        '`{{key}}` is passed by direct reference: with an import cycle it becomes undefined in the prod bundle, and the abort signal is lost. Wrap it: `({ signal }) => fetchX(params, signal)`.',
    },
    schema: [],
  },
  create(context) {
    return {
      Property(node) {
        if (node.key.type !== 'Identifier' || !LAZY_KEYS.has(node.key.name)) return;
        if (node.value.type === 'Identifier' || node.value.type === 'MemberExpression') {
          if (node.value.type === 'Identifier' && node.value.name === 'skipToken') return;
          context.report({ node: node.value, messageId: 'direct', data: { key: node.key.name } });
        }
      },
    };
  },
};
