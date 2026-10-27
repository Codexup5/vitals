const BANNER = /^[\s*/=#-]{4,}$|^\s*[=#*-]{3,}.*[=#*-]{3,}\s*$/;
const COMMENTED_CODE = /^\s*(import |export |const |let |return |await |if \(|for \(|}\s*$|.*;\s*$|<\/?[A-Z][\w.]*)/;
const TASK_MARKER = /\b(TODO|FIXME|HACK|XXX)\b/;
const TASK_REFERENCE = /\b(TODO|FIXME|HACK|XXX)\s*\(?\s*([A-Z][A-Z0-9]+-\d+|#\d+)/;
const TOOL_DIRECTIVE = /^\s*(eslint|prettier-ignore|@ts-|istanbul|c8|global |vite-ignore|webpackChunkName)/;

const isCommentedCode = (comment) => {
  if (comment.type === 'Line') return COMMENTED_CODE.test(comment.value);
  if (comment.value.startsWith('*')) return false;
  return comment.value.split(/\r?\n/).some((line) => COMMENTED_CODE.test(line.replace(/^[\s*]+/, '')));
};

export default {
  meta: {
    type: 'suggestion',
    docs: { description: 'No commented-out code, banners or untracked TODOs' },
    messages: {
      code: 'Commented-out code. It is in git history: delete it.',
      banner: 'Banner divider. Functions and files give a file its structure, not a line.',
      todo: '`{{marker}}` without a ticket. Give a number (`TODO(PROJ-123)`) or file a ticket instead of the comment.',
    },
    schema: [],
  },
  create(context) {
    return {
      Program() {
        for (const comment of context.sourceCode.getAllComments()) {
          const text = comment.value;
          if (TOOL_DIRECTIVE.test(text)) continue;
          const marker = TASK_MARKER.exec(text);
          if (marker && !TASK_REFERENCE.test(text)) {
            context.report({ loc: comment.loc, messageId: 'todo', data: { marker: marker[1] } });
            continue;
          }
          if (BANNER.test(text)) {
            context.report({ loc: comment.loc, messageId: 'banner' });
            continue;
          }
          if (isCommentedCode(comment)) context.report({ loc: comment.loc, messageId: 'code' });
        }
      },
    };
  },
};
