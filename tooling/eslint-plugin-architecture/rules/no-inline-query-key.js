const CACHE_METHODS = new Set(['getQueryData', 'setQueryData', 'getQueriesData', 'setQueriesData']);
const WRAPPERS = new Set(['TSAsExpression', 'TSSatisfiesExpression', 'TSNonNullExpression']);

const unwrap = (node) => {
  let current = node;
  while (current && WRAPPERS.has(current.type)) current = current.expression;
  return current;
};

export default {
  meta: {
    type: 'problem',
    docs: { description: 'Query keys come from the entity key factory, never from an inline array' },
    messages: {
      inline:
        'The query key is built inline: invalidation will silently miss. Take the key from the `{ENTITY}_KEYS` factory of the entity.',
    },
    schema: [],
  },
  create(context) {
    return {
      Property(node) {
        const isQueryKey = node.key.type === 'Identifier' && node.key.name === 'queryKey';
        if (isQueryKey && unwrap(node.value)?.type === 'ArrayExpression')
          context.report({ node: node.value, messageId: 'inline' });
      },
      CallExpression(node) {
        const { callee } = node;
        const method =
          callee.type === 'MemberExpression' && callee.property.type === 'Identifier' ? callee.property.name : null;
        if (method && CACHE_METHODS.has(method) && unwrap(node.arguments[0])?.type === 'ArrayExpression') {
          context.report({ node: node.arguments[0], messageId: 'inline' });
        }
      },
    };
  },
};
