import { isEntityModel, isTestFile, projectPath } from '../lib/paths.js';

const FACTORIES = new Set(['queryOptions', 'infiniteQueryOptions', 'mutationOptions', 'useMutation']);
const PROXY_HOOKS = new Set(['useTRPC', 'useTRPCClient']);
const READERS = new Set([
  'useQuery',
  'useSuspenseQuery',
  'useInfiniteQuery',
  'useSuspenseInfiniteQuery',
  'usePrefetchQuery',
  'usePrefetchInfiniteQuery',
]);
const CLIENT_READERS = new Set([
  'query',
  'prefetchQuery',
  'ensureQueryData',
  'fetchQuery',
  'infiniteQuery',
  'prefetchInfiniteQuery',
  'fetchInfiniteQuery',
  'ensureInfiniteQueryData',
]);
const QUERIES_HOOKS = new Set(['useQueries', 'useSuspenseQueries']);
const CACHE_WRITES = new Set([
  'invalidateQueries',
  'refetchQueries',
  'resetQueries',
  'removeQueries',
  'cancelQueries',
  'setQueryData',
  'setQueriesData',
]);

const isQueryObjectCall = (argument) => {
  if (!argument) return false;
  if (argument.type === 'CallExpression') return true;
  if (argument.type !== 'ObjectExpression') return false;
  return argument.properties.some(
    (property) => property.type === 'SpreadElement' && property.argument.type === 'CallExpression',
  );
};

const isInlineQueryObject = (element) => element?.type === 'ObjectExpression' && !isQueryObjectCall(element);

const getQueryElements = (queries) => {
  if (queries?.type === 'ArrayExpression') return queries.elements;
  const isMapCall =
    queries?.type === 'CallExpression' &&
    queries.callee.type === 'MemberExpression' &&
    queries.callee.property.type === 'Identifier' &&
    queries.callee.property.name === 'map';
  const mapper = isMapCall ? queries.arguments[0] : null;
  if (mapper?.type === 'ArrowFunctionExpression' && mapper.body.type !== 'BlockStatement') return [mapper.body];
  return [];
};

const getQueriesProperty = (argument) =>
  argument?.type === 'ObjectExpression'
    ? argument.properties.find(
        (property) =>
          property.type === 'Property' && property.key.type === 'Identifier' && property.key.name === 'queries',
      )
    : undefined;

export default {
  meta: {
    type: 'problem',
    docs: { description: 'Queries and mutations are declared once, in the model segment of an entity' },
    messages: {
      factory:
        '`{{name}}` outside `entities/*/model`. A domain query and mutation are declared once in the entity, so key, transport and invalidation do not drift apart.',
      inline:
        'The query object is built inline. Pass `xQuery(params)` from the entity (fields can be overridden with a spread: `{ ...xQuery(id), enabled }`).',
      cache:
        'A cache write outside the model of an entity: call the cache event of the entity, such as `putXInCache` or `dropXFromCache`.',
      proxy:
        'The tRPC options proxy is read only in the model of an entity: call the query object or the mutation hook of the entity.',
    },
    schema: [],
  },
  create(context) {
    if (isTestFile(projectPath(context))) return {};
    const isInEntityModel = isEntityModel(projectPath(context));

    return {
      CallExpression(node) {
        const { callee } = node;
        const name =
          callee.type === 'Identifier'
            ? callee.name
            : callee.type === 'MemberExpression' && callee.property.type === 'Identifier'
              ? callee.property.name
              : null;
        if (!name) return;
        if (PROXY_HOOKS.has(name) && !isInEntityModel) context.report({ node, messageId: 'proxy' });
        if (FACTORIES.has(name) && !isInEntityModel) context.report({ node, messageId: 'factory', data: { name } });
        if (
          (READERS.has(name) || CLIENT_READERS.has(name)) &&
          !isInEntityModel &&
          !isQueryObjectCall(node.arguments[0])
        ) {
          context.report({ node, messageId: 'inline' });
        }
        if (QUERIES_HOOKS.has(name) && !isInEntityModel) {
          const queries = getQueriesProperty(node.arguments[0])?.value;
          for (const element of getQueryElements(queries)) {
            if (isInlineQueryObject(element)) context.report({ node: element, messageId: 'inline' });
          }
        }
        if (CACHE_WRITES.has(name) && !isInEntityModel) context.report({ node, messageId: 'cache' });
      },
    };
  },
};
