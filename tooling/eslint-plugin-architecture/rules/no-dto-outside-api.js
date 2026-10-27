import { isInSegment, projectPath } from '../lib/paths.js';

const DTO_MODULE = /[-.]dto$/;

export default {
  meta: {
    type: 'problem',
    docs: { description: 'Wire format (DTO) never leaves the api segment' },
    messages: {
      leak: 'The backend format (`{{source}}`) is imported outside `api/`. Above the api segment only the domain model lives: convert it through the mapper.',
    },
    schema: [],
  },
  create(context) {
    if (isInSegment(projectPath(context), 'api')) return {};
    const check = (node) => {
      if (!node.source || node.source.type !== 'Literal') return;
      const source = String(node.source.value);
      if (DTO_MODULE.test(source)) context.report({ node, messageId: 'leak', data: { source } });
    };
    return {
      ImportDeclaration: check,
      ExportNamedDeclaration: check,
      ExportAllDeclaration: check,
      ImportExpression: check,
    };
  },
};
