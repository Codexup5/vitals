import { isTestFile, projectPath, toSrcPath } from '../lib/paths.js';

export default {
  meta: {
    type: 'problem',
    docs: { description: 'A button outside shared/ui is the shared Button, not a raw button element' },
    messages: {
      raw: 'A raw button outside shared/ui: use Button from @/shared/ui/button/Button',
    },
    schema: [],
  },
  create(context) {
    const filename = projectPath(context);
    const srcPath = toSrcPath(filename);
    if (srcPath === null || !filename.endsWith('.tsx') || srcPath.startsWith('shared/ui/') || isTestFile(filename)) {
      return {};
    }

    return {
      JSXOpeningElement(node) {
        if (node.name.type === 'JSXIdentifier' && node.name.name === 'button') {
          context.report({ node, messageId: 'raw' });
        }
      },
    };
  },
};
