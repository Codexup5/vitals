import { isEntityModel, isTestFile, projectPath, toSrcPath } from '../lib/paths.js';

const ID_NAME = /^(?:id|.*Id)$/;
const OPTIONAL_FACTORY = /ByOptionalIdQuery$/;

const isEmptyString = (node) => node.type === 'Literal' && node.value === '';

const isIdReference = (node) => {
  if (node.type === 'Identifier') return ID_NAME.test(node.name);
  if (node.type === 'MemberExpression' && !node.computed && node.property.type === 'Identifier') {
    return ID_NAME.test(node.property.name);
  }
  return false;
};

const isUndefinedUnion = (annotation) =>
  annotation?.typeAnnotation.type === 'TSUnionType' &&
  annotation.typeAnnotation.types.some((type) => type.type === 'TSUndefinedKeyword');

const functionName = (node) => {
  if (node.id) return node.id.name;
  const { parent } = node;
  return parent.type === 'VariableDeclarator' && parent.id.type === 'Identifier' ? parent.id.name : '';
};

export default {
  meta: {
    type: 'problem',
    docs: { description: 'An id parameter is required, never optional, undefined or defaulted to an empty string' },
    messages: {
      optional: 'An `id` or `*Id` parameter is required: take `string`, and wrap a dependent query in a factory.',
      emptyDefault: 'Do not default an id to an empty string: an empty id is a request for nothing.',
      emptyFallback: 'Do not fall back an id to an empty string: let the caller handle the absent id.',
    },
    schema: [],
  },
  create(context) {
    const filename = projectPath(context);
    if (toSrcPath(filename) === null || isTestFile(filename)) return {};
    const inEntityModel = isEntityModel(filename);

    const checkFunction = (node) => {
      const allowsOptional = inEntityModel && OPTIONAL_FACTORY.test(functionName(node));
      for (const param of node.params) {
        const target = param.type === 'AssignmentPattern' ? param.left : param;
        if (target.type !== 'Identifier' || !ID_NAME.test(target.name)) continue;
        if (param.type === 'AssignmentPattern' && isEmptyString(param.right)) {
          context.report({ node: param, messageId: 'emptyDefault' });
        }
        if (!allowsOptional && (target.optional || isUndefinedUnion(target.typeAnnotation))) {
          context.report({ node: param, messageId: 'optional' });
        }
      }
    };

    return {
      FunctionDeclaration: checkFunction,
      FunctionExpression: checkFunction,
      ArrowFunctionExpression: checkFunction,
      LogicalExpression(node) {
        const fallsBack = node.operator === '??' || node.operator === '||';
        if (fallsBack && isEmptyString(node.right) && isIdReference(node.left)) {
          context.report({ node, messageId: 'emptyFallback' });
        }
      },
    };
  },
};
