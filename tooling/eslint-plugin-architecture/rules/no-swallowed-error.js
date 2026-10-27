const isFunctionNode = (node) => node.type === 'ArrowFunctionExpression' || node.type === 'FunctionExpression';

const hasThen = (services, node) => {
  const checker = services.program.getTypeChecker();
  const type = checker.getNonNullableType(checker.getTypeAtLocation(services.esTreeNodeToTSNodeMap.get(node)));
  return checker.getPropertyOfType(type, 'then') !== undefined;
};

export default {
  meta: {
    type: 'problem',
    docs: { description: 'A catch block names the error and does something with it' },
    messages: {
      noParam:
        '`catch` without a parameter turns network, 500 and a broken schema into one silent fallback. Name the error and narrow it.',
      unused: 'Error `{{name}}` is caught and unused: it is swallowed. Narrow it (`isApiError`), show it or rethrow.',
    },
    schema: [],
  },
  create(context) {
    const { sourceCode } = context;
    const services = sourceCode.parserServices;
    const isTyped = Boolean(services?.program);

    return {
      CallExpression(node) {
        const { callee } = node;
        if (callee.type !== 'MemberExpression' || callee.computed || callee.property.name !== 'catch') return;
        const [handler] = node.arguments;
        if (!handler || !isFunctionNode(handler)) return;
        if (isTyped && !hasThen(services, callee.object)) return;
        const [param] = handler.params;
        if (!param) {
          context.report({ node, messageId: 'noParam' });
          return;
        }
        if (param.type !== 'Identifier') return;
        const variable = sourceCode.getDeclaredVariables(handler).find(({ name }) => name === param.name);
        if (variable && variable.references.length === 0) {
          context.report({ node: param, messageId: 'unused', data: { name: param.name } });
        }
      },
      CatchClause(node) {
        if (!node.param) {
          context.report({ node, messageId: 'noParam' });
          return;
        }
        if (node.param.type !== 'Identifier') return;
        const [variable] = sourceCode.getDeclaredVariables(node);
        if (variable && variable.references.length === 0) {
          context.report({ node: node.param, messageId: 'unused', data: { name: node.param.name } });
        }
      },
    };
  },
};
