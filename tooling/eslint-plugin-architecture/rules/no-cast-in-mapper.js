import { isInSegment, isTestFile, projectPath } from '../lib/paths.js';

const isConstAssertion = (node) =>
  node.typeAnnotation.type === 'TSTypeReference' &&
  node.typeAnnotation.typeName.type === 'Identifier' &&
  node.typeAnnotation.typeName.name === 'const';

export default {
  meta: {
    type: 'problem',
    docs: { description: 'The api segment builds the object instead of persuading the compiler' },
    messages: {
      cast: '`as` in `api/` disables the only object-completeness check: a missed field will show up as an empty cell on screen, not as a compiler error.',
    },
    schema: [],
  },
  create(context) {
    if (!isInSegment(projectPath(context), 'api') || isTestFile(projectPath(context))) return {};
    const report = (node) => {
      if (!isConstAssertion(node)) context.report({ node, messageId: 'cast' });
    };
    return { TSAsExpression: report, TSTypeAssertion: report };
  },
};
