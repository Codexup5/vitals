import { isInSegment, projectPath } from '../lib/paths.js';

const LOGIC_HOOKS = new Set([
  'useEffect',
  'useLayoutEffect',
  'useMemo',
  'useCallback',
  'useReducer',
  'useQuery',
  'useSuspenseQuery',
  'useInfiniteQuery',
  'useMutation',
  'useQueryClient',
]);
const BEHAVIOUR_HOOKS = new Set(['useEffect', 'useLayoutEffect', 'useMemo', 'useCallback', 'useReducer']);
const STORAGES = new Set(['localStorage', 'sessionStorage']);
const CALCULATIONS = new Set(['reduce', 'reduceRight']);
const MAX_USE_STATE = 2;

const isMember = (callee) =>
  callee.type === 'MemberExpression' && !callee.computed && callee.property.type === 'Identifier';

const calleeName = (callee) => {
  if (callee.type === 'Identifier') return callee.name;
  if (isMember(callee) && callee.object.type === 'Identifier' && callee.object.name === 'React') {
    return callee.property.name;
  }
  return null;
};

export default {
  meta: {
    type: 'problem',
    docs: { description: 'Components in ui/ render; effects, queries and storage live in model/' },
    messages: {
      hook: '`{{name}}` in ui/ is logic. Move it to a `model/use-*.ts` hook; the component receives ready values.',
      fetch:
        'A request from a component. Transport lives in the `api/` of the entity, the call goes through queryOptions in `model/`.',
      storage:
        '`{{name}}` from a component. Storage access belongs in a `model/` hook, with the side effect in its name.',
      iife: 'An IIFE in markup is hidden branching. Move it to a named `lib/` function or into the output of the `model/` hook.',
      calculation:
        '`{{name}}` in ui/ is a calculation: move it into a `lib/` function or the view model of the `model/` hook.',
      tooManyStates: 'More than {{max}} `useState` in a component is screen state; it belongs in a `model/` hook.',
    },
    schema: [],
  },
  create(context) {
    const filename = projectPath(context);
    if (!isInSegment(filename, 'ui')) return {};
    const isSharedUi = filename.includes('/src/shared/ui/');
    const useStateCounts = [];

    const enterFunction = () => useStateCounts.push(0);
    const exitFunction = () => useStateCounts.pop();

    return {
      FunctionDeclaration: enterFunction,
      FunctionExpression: enterFunction,
      ArrowFunctionExpression: enterFunction,
      'FunctionDeclaration:exit': exitFunction,
      'FunctionExpression:exit': exitFunction,
      'ArrowFunctionExpression:exit': exitFunction,
      CallExpression(node) {
        const { callee } = node;
        const name = calleeName(callee);
        if (isMember(callee) && CALCULATIONS.has(callee.property.name)) {
          context.report({ node, messageId: 'calculation', data: { name: callee.property.name } });
        }
        if (name && LOGIC_HOOKS.has(name) && !(isSharedUi && BEHAVIOUR_HOOKS.has(name))) {
          context.report({ node, messageId: 'hook', data: { name } });
        }
        if (name === 'fetch') context.report({ node, messageId: 'fetch' });
        if (callee.type === 'ArrowFunctionExpression' || callee.type === 'FunctionExpression') {
          context.report({ node, messageId: 'iife' });
        }
        if (name === 'useState' && !isSharedUi && useStateCounts.length > 0) {
          const index = useStateCounts.length - 1;
          useStateCounts[index] += 1;
          if (useStateCounts[index] === MAX_USE_STATE + 1) {
            context.report({ node, messageId: 'tooManyStates', data: { max: String(MAX_USE_STATE) } });
          }
        }
      },
      Identifier(node) {
        if (STORAGES.has(node.name) && node.parent.type !== 'Property') {
          context.report({ node, messageId: 'storage', data: { name: node.name } });
        }
      },
    };
  },
};
