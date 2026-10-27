import { projectPath, resolveFromSrc, toSrcPath } from '../lib/paths.js';

const SLICE_SEGMENT = /^(?:pages\/(?:[^/]+\/)?|(?:features|entities)\/)[^/]+\/(lib|model)\//;
const UI_SEGMENT = /^(?:pages\/(?:[^/]+\/)?|(?:features|entities)\/)[^/]+\/ui(?:\/|$)/;
const HOOK_NAME = /^use[A-Z]/;

const importedName = (specifier) => {
  if (specifier.type !== 'ImportSpecifier') return '';
  return specifier.imported.type === 'Identifier' ? specifier.imported.name : String(specifier.imported.value);
};

export default {
  meta: {
    type: 'problem',
    docs: { description: 'The lib and model segments of a slice do not import ui, and lib stays free of React' },
    messages: {
      ui: 'The `lib` and `model` segments do not import `ui`: move the shared piece down to `lib` or `model`.',
      react: 'The `lib` segment holds pure functions: it imports no React value, only types.',
      hook: 'The `lib` segment holds pure functions: a hook lives in `model`.',
    },
    schema: [],
  },
  create(context) {
    const filename = projectPath(context);
    const own = toSrcPath(filename);
    const match = own === null ? null : SLICE_SEGMENT.exec(own);
    if (!match) return {};
    const isLib = match[1] === 'lib';

    const importsUi = (node) => {
      if (!node.source || node.source.type !== 'Literal' || typeof node.source.value !== 'string') return false;
      const resolved = resolveFromSrc(filename, node.source.value);
      return resolved !== null && UI_SEGMENT.test(resolved);
    };

    const checkUi = (node) => {
      if (importsUi(node)) context.report({ node, messageId: 'ui' });
    };

    const checkImport = (node) => {
      if (importsUi(node)) {
        context.report({ node, messageId: 'ui' });
        return;
      }
      if (!isLib || node.importKind === 'type') return;
      const values = node.specifiers.filter((specifier) => specifier.importKind !== 'type');
      if (node.source.value === 'react' && values.length > 0) {
        context.report({ node, messageId: 'react' });
      } else if (values.some((specifier) => HOOK_NAME.test(importedName(specifier)))) {
        context.report({ node, messageId: 'hook' });
      }
    };

    return {
      ImportDeclaration: checkImport,
      ExportNamedDeclaration: checkUi,
      ExportAllDeclaration: checkUi,
      ImportExpression: checkUi,
    };
  },
};
