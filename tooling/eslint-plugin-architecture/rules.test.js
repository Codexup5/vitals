import { readdirSync, readFileSync } from 'node:fs';
import { Linter, RuleTester } from 'eslint';
import tseslint from 'typescript-eslint';
import { afterAll, describe, expect, it } from 'vitest';

import { rules } from './index.js';

RuleTester.afterAll = afterAll;
RuleTester.describe = describe;
RuleTester.it = it;
RuleTester.itOnly = it.only;

const tester = new RuleTester({
  languageOptions: {
    parser: tseslint.parser,
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
});

const UI_FILE = '/repo/src/pages/user-list/ui/UserListPage.tsx';
const MODEL_FILE = '/repo/src/pages/user-list/model/use-user-list-page.ts';
const ENTITY_MODEL_FILE = '/repo/src/entities/user/model/user-queries.ts';
const SHARED_UI_FILE = '/repo/src/shared/ui/sheet/Sheet.tsx';
const LIB_FILE = '/repo/src/pages/user-list/lib/user-list-view.ts';
const GROUP_LIB_FILE = '/repo/src/pages/guide/help/lib/help-view.ts';

tester.run('no-inline-query-key', rules['no-inline-query-key'], {
  valid: [
    'queryOptions({ queryKey: USER_KEYS.detail(id), queryFn: () => f() })',
    'queryClient.invalidateQueries({ queryKey: USER_KEYS.lists() })',
    'const USER_KEYS = { all: () => ["user"] as const }',
    'useQuery({ queryKey: USER_KEYS.detail(id) as QueryKey })',
  ],
  invalid: [
    { code: 'useQuery({ queryKey: ["user", id] })', errors: [{ messageId: 'inline' }] },
    { code: 'queryClient.setQueryData(["user", id], user)', errors: [{ messageId: 'inline' }] },
    { code: 'useQuery({ queryKey: ["user", id] as const })', errors: [{ messageId: 'inline' }] },
    { code: 'useQuery({ queryKey: ["user", id] satisfies QueryKey })', errors: [{ messageId: 'inline' }] },
    { code: 'useQuery({ queryKey: ["user", id]! })', errors: [{ messageId: 'inline' }] },
    { code: 'queryClient.setQueryData(["user", id] as const, user)', errors: [{ messageId: 'inline' }] },
  ],
});

tester.run('lazy-query-fn', rules['lazy-query-fn'], {
  valid: [
    'queryOptions({ queryFn: ({ signal }) => fetchUser(id, signal) })',
    'useQuery({ ...userQuery(id), queryFn: skipToken })',
  ],
  invalid: [
    { code: 'queryOptions({ queryFn: fetchUsers })', errors: [{ messageId: 'direct' }] },
    { code: 'useMutation({ mutationFn: api.deleteUser })', errors: [{ messageId: 'direct' }] },
  ],
});

tester.run('no-swallowed-error', rules['no-swallowed-error'], {
  valid: [
    'try { f() } catch (error) { if (!isApiError(error)) throw error; }',
    'promise.catch(reportError)',
    'promise.catch((error) => report(error))',
    'promise.catch(function onError(error) { report(error); })',
  ],
  invalid: [
    { code: 'promise.catch(function onError(error) { return 0; })', errors: [{ messageId: 'unused' }] },
    { code: 'try { f() } catch { fallback() }', errors: [{ messageId: 'noParam' }] },
    { code: 'try { f() } catch (error) { fallback() }', errors: [{ messageId: 'unused' }] },
    { code: 'promise.catch(() => fallback())', errors: [{ messageId: 'noParam' }] },
    { code: 'promise.catch((error) => fallback())', errors: [{ messageId: 'unused' }] },
  ],
});

tester.run('no-logic-in-ui', rules['no-logic-in-ui'], {
  valid: [
    {
      code: 'const A = () => { const view = useScreen(); const [isOpen] = useState(false); return null; }',
      filename: UI_FILE,
    },
    { code: 'useEffect(() => {}, [])', filename: MODEL_FILE },
    { code: 'const A = () => { const id = React.useId(); return null; }', filename: UI_FILE },
    { code: 'const A = () => <ul>{rows.map((row) => <li key={row.id} />)}</ul>', filename: UI_FILE },
    { code: 'const total = rows.reduce((sum, row) => sum + row.amount, 0)', filename: MODEL_FILE },
    {
      code: 'const A = () => { useEffect(() => {}, []); return null; }',
      filename: SHARED_UI_FILE,
    },
    {
      code: 'const A = () => { useLayoutEffect(() => {}, []); return null; }',
      filename: SHARED_UI_FILE,
    },
    { code: 'const A = () => { const v = useMemo(() => 1, []); return null; }', filename: SHARED_UI_FILE },
    {
      code: 'const A = () => { const f = useCallback(() => {}, []); return null; }',
      filename: SHARED_UI_FILE,
    },
    {
      code: 'const A = () => { const [s, dispatch] = useReducer(reducer, 0); return null; }',
      filename: SHARED_UI_FILE,
    },
    {
      code: 'const A = () => { useState(1); useState(2); useState(3); return null; }',
      filename: SHARED_UI_FILE,
    },
  ],
  invalid: [
    {
      code: 'const A = () => { React.useEffect(() => {}, []); return null; }',
      filename: UI_FILE,
      errors: [{ messageId: 'hook' }],
    },
    {
      code: 'const A = () => { React.useState(1); React.useState(2); React.useState(3); return null; }',
      filename: UI_FILE,
      errors: [{ messageId: 'tooManyStates' }],
    },
    {
      code: 'const A = () => <p>{rows.reduce((sum, row) => sum + row.amount, 0)}</p>',
      filename: UI_FILE,
      errors: [{ messageId: 'calculation' }],
    },
    {
      code: 'const A = () => <p>{rows.reduceRight((sum, row) => sum + row.amount, 0)}</p>',
      filename: UI_FILE,
      errors: [{ messageId: 'calculation' }],
    },
    {
      code: 'const A = () => { useEffect(() => {}, []); return null; }',
      filename: UI_FILE,
      errors: [{ messageId: 'hook' }],
    },
    {
      code: 'const A = () => { const q = useQuery(x()); return null; }',
      filename: UI_FILE,
      errors: [{ messageId: 'hook' }],
    },
    {
      code: 'const A = () => { localStorage.getItem("k"); return null; }',
      filename: UI_FILE,
      errors: [{ messageId: 'storage' }],
    },
    { code: 'const A = () => <div>{(() => 1)()}</div>', filename: UI_FILE, errors: [{ messageId: 'iife' }] },
    {
      code: 'const A = () => { useState(1); useState(2); useState(3); return null; }',
      filename: UI_FILE,
      errors: [{ messageId: 'tooManyStates' }],
    },
    {
      code: 'const A = () => { const q = useQuery(x()); return null; }',
      filename: SHARED_UI_FILE,
      errors: [{ messageId: 'hook' }],
    },
    {
      code: 'const A = () => { const m = useMutation(x()); return null; }',
      filename: SHARED_UI_FILE,
      errors: [{ messageId: 'hook' }],
    },
    {
      code: 'const A = () => { const client = useQueryClient(); return null; }',
      filename: SHARED_UI_FILE,
      errors: [{ messageId: 'hook' }],
    },
    {
      code: 'const A = () => { fetch("/api"); return null; }',
      filename: SHARED_UI_FILE,
      errors: [{ messageId: 'fetch' }],
    },
    {
      code: 'const A = () => { localStorage.getItem("k"); return null; }',
      filename: SHARED_UI_FILE,
      errors: [{ messageId: 'storage' }],
    },
    { code: 'const A = () => <div>{(() => 1)()}</div>', filename: SHARED_UI_FILE, errors: [{ messageId: 'iife' }] },
    {
      code: 'const A = () => <p>{rows.reduce((sum, row) => sum + row.amount, 0)}</p>',
      filename: SHARED_UI_FILE,
      errors: [{ messageId: 'calculation' }],
    },
  ],
});

tester.run('query-options-in-entity', rules['query-options-in-entity'], {
  valid: [
    {
      code: 'export const q = () => queryOptions({ queryKey: K.all(), queryFn: () => f() })',
      filename: ENTITY_MODEL_FILE,
    },
    { code: 'useQuery(userListQuery(params))', filename: MODEL_FILE },
    { code: 'useQuery({ ...userByIdQuery(id), enabled: false })', filename: MODEL_FILE },
    { code: 'queryClient.ensureQueryData(userByIdQuery(id))', filename: '/repo/src/app/routing/router.tsx' },
    {
      code: 'useQueries({ queries: ids.map((id) => userByIdQuery(id)) })',
      filename: MODEL_FILE,
    },
    {
      code: 'useQueries({ queries: [userByIdQuery(1), { ...userByIdQuery(2), enabled: false }] })',
      filename: MODEL_FILE,
    },
    {
      code: 'queryClient.invalidateQueries({ queryKey: USER_KEYS.lists() })',
      filename: ENTITY_MODEL_FILE,
    },
    {
      code: 'queryClient.setQueryData(USER_KEYS.detail(id), user)',
      filename: ENTITY_MODEL_FILE,
    },
    {
      code: 'queryClient.removeQueries({ queryKey: USER_KEYS.detail(id) })',
      filename: '/repo/src/pages/user-list/model/use-user-list-page.test.ts',
    },
    { code: 'queryClient.getQueryData(USER_KEYS.detail(id))', filename: MODEL_FILE },
    { code: 'queryClient.clear()', filename: MODEL_FILE },
    {
      code: "context.queryClient.query({ ...userByIdQuery(id), staleTime: 'static' })",
      filename: '/repo/src/app/routing/router.tsx',
    },
    {
      code: 'const trpc = useTRPC(); trpc.user.byId.queryOptions({ id });',
      filename: ENTITY_MODEL_FILE,
    },
    {
      code: 'const trpc = useTRPC(); useMutation(trpc.user.create.mutationOptions({ onSuccess }));',
      filename: ENTITY_MODEL_FILE,
    },
    {
      code: 'const trpc = useTRPC(); queryClient.invalidateQueries({ queryKey: trpc.user.list.pathKey() });',
      filename: ENTITY_MODEL_FILE,
    },
    {
      code: 'const trpc = createTRPCOptionsProxy({ client, queryClient }); export const userByIdQuery = (id) => trpc.user.byId.queryOptions({ id });',
      filename: ENTITY_MODEL_FILE,
    },
    { code: 'useQuery(userByIdQuery(id))', filename: MODEL_FILE },
    { code: 'useSuspenseQuery(userByIdQuery(id))', filename: MODEL_FILE },
    { code: 'const trpc = useTRPC();', filename: '/repo/src/pages/user-list/model/use-user-list-page.test.ts' },
  ],
  invalid: [
    { code: 'const trpc = useTRPC();', filename: MODEL_FILE, errors: [{ messageId: 'proxy' }] },
    { code: 'const client = useTRPCClient();', filename: MODEL_FILE, errors: [{ messageId: 'proxy' }] },
    {
      code: 'trpc.user.byId.queryOptions({ id })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'factory' }],
    },
    {
      code: 'trpc.user.create.mutationOptions()',
      filename: MODEL_FILE,
      errors: [{ messageId: 'factory' }],
    },
    {
      code: 'useQuery(trpc.user.byId.queryOptions({ id }))',
      filename: MODEL_FILE,
      errors: [{ messageId: 'factory' }],
    },
    {
      code: 'queryClient.invalidateQueries({ queryKey: trpc.user.list.pathKey() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'cache' }],
    },
    {
      code: 'useQueries({ queries: [{ queryKey: K.all(), queryFn: () => f() }] })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'useSuspenseQueries({ queries: [{ queryKey: K.all(), queryFn: () => f() }] })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'useQueries({ queries: ids.map((id) => ({ queryKey: K.detail(id), queryFn: () => f(id) })) })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'usePrefetchQuery({ queryKey: K.all(), queryFn: () => f() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'usePrefetchInfiniteQuery({ queryKey: K.all(), queryFn: () => f() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'queryClient.infiniteQuery({ queryKey: K.all(), queryFn: () => f() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'queryClient.prefetchInfiniteQuery({ queryKey: K.all(), queryFn: () => f() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'queryClient.fetchInfiniteQuery({ queryKey: K.all(), queryFn: () => f() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'queryClient.ensureInfiniteQueryData({ queryKey: K.all(), queryFn: () => f() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'queryClient.invalidateQueries({ queryKey: USER_KEYS.lists() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'cache' }],
    },
    {
      code: 'queryClient.refetchQueries({ queryKey: USER_KEYS.lists() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'cache' }],
    },
    {
      code: 'queryClient.resetQueries({ queryKey: USER_KEYS.lists() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'cache' }],
    },
    {
      code: 'queryClient.removeQueries({ queryKey: USER_KEYS.lists() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'cache' }],
    },
    {
      code: 'queryClient.cancelQueries({ queryKey: USER_KEYS.lists() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'cache' }],
    },
    {
      code: 'queryClient.setQueryData(USER_KEYS.detail(id), user)',
      filename: MODEL_FILE,
      errors: [{ messageId: 'cache' }],
    },
    {
      code: 'queryClient.setQueriesData({ queryKey: USER_KEYS.lists() }, updater)',
      filename: MODEL_FILE,
      errors: [{ messageId: 'cache' }],
    },
    {
      code: 'queryOptions({ queryKey: K.all(), queryFn: () => f() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'factory' }],
    },
    { code: 'useMutation({ mutationFn: () => f() })', filename: MODEL_FILE, errors: [{ messageId: 'factory' }] },
    {
      code: 'useQuery({ queryKey: K.all(), queryFn: () => f() })',
      filename: MODEL_FILE,
      errors: [{ messageId: 'inline' }],
    },
  ],
});

tester.run('no-dto-outside-api', rules['no-dto-outside-api'], {
  valid: [
    { code: 'import type { UserDto } from "./user-dto";', filename: '/repo/src/entities/user/api/user-mapper.ts' },
    { code: 'export type { UserDto } from "./user-dto";', filename: '/repo/src/entities/user/api/user-mapper.ts' },
    { code: 'export * from "../api/user-dto";', filename: '/repo/src/entities/user/api/user-mapper.ts' },
    { code: 'const load = () => import("../api/user-dto");', filename: '/repo/src/entities/user/api/user-mapper.ts' },
  ],
  invalid: [
    {
      code: 'import type { UserDto } from "../api/user-dto";',
      filename: '/repo/src/entities/user/model/user.ts',
      errors: [{ messageId: 'leak' }],
    },
    {
      code: 'export type { UserDto } from "../api/user-dto";',
      filename: '/repo/src/entities/user/model/user.ts',
      errors: [{ messageId: 'leak' }],
    },
    {
      code: 'export * from "../api/user-dto";',
      filename: '/repo/src/entities/user/model/user.ts',
      errors: [{ messageId: 'leak' }],
    },
    {
      code: 'const load = () => import("../api/user-dto");',
      filename: '/repo/src/entities/user/model/user.ts',
      errors: [{ messageId: 'leak' }],
    },
  ],
});

tester.run('no-cast-in-mapper', rules['no-cast-in-mapper'], {
  valid: [
    {
      code: 'export const toUser = (dto) => ({ id: dto.id, kind: "user" as const })',
      filename: '/repo/src/entities/user/api/user-mapper.ts',
    },
    {
      code: 'export const USER_PATH = "/users" as const',
      filename: '/repo/src/entities/user/api/user-api.ts',
    },
    { code: 'const x = y as User', filename: '/repo/src/entities/user/api/user-mapper.test.ts' },
    { code: 'const x = y as User', filename: '/repo/src/entities/user/model/user.ts' },
  ],
  invalid: [
    {
      code: 'const x = y as User',
      filename: '/repo/src/shared/api/http-client.ts',
      errors: [{ messageId: 'cast' }],
    },
    {
      code: 'export const getUser = async (r) => r as UserDto',
      filename: '/repo/src/entities/user/api/user-api.ts',
      errors: [{ messageId: 'cast' }],
    },
    {
      code: 'export const toUser = (dto) => dto as User',
      filename: '/repo/src/entities/user/api/user-mapper.ts',
      errors: [{ messageId: 'cast' }],
    },
  ],
});

tester.run('no-junk-comments', rules['no-junk-comments'], {
  valid: [
    '// Safari does not expose visualViewport until the first resize, so we read it after the event.\nconst a = 1;',
    '// TODO(PROJ-42): remove after the backend migration\nconst a = 1;',
    '// eslint-disable-next-line no-console -- debug output in dev only\nconsole.log(1);',
    '/** @example const x = 1; */\nconst a = 1;',
    '/* Safari does not expose visualViewport\n   until the first resize. */\nconst a = 1;',
  ],
  invalid: [
    { code: '// const legacy = compute();\nconst a = 1;', errors: [{ messageId: 'code' }] },
    { code: '// ===== Helpers =====\nconst a = 1;', errors: [{ messageId: 'banner' }] },
    { code: '// TODO fix later\nconst a = 1;', errors: [{ messageId: 'todo' }] },
    {
      code: '/*\nconst legacy = compute();\nreturn legacy;\n*/\nconst a = 1;',
      errors: [{ messageId: 'code' }],
    },
  ],
});

tester.run('no-inline-style', rules['no-inline-style'], {
  valid: [
    'const a = <div style={{ "--progress": "40%" }} />',
    'const a = <div className={styles.box} />',
    'const style = { "--company": fill } as CSSProperties;\nconst a = <div style={style} />',
    'const a = <div style={ring ? ({ "--avatar-ring": ring } as CSSProperties) : undefined} />',
    'const a = <div style={{ "--progress": value } satisfies CSSProperties} />',
  ],
  invalid: [
    { code: 'const a = <div style={{ color: "red" }} />', errors: [{ messageId: 'inline' }] },
    { code: 'const a = <div style={dynamicStyle} />', errors: [{ messageId: 'inline' }] },
    {
      code: 'const style = { color: "red" };\nconst a = <div style={style} />',
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'const a = <div style={ring ? { "--avatar-ring": ring } : { color: "red" }} />',
      errors: [{ messageId: 'inline' }],
    },
    {
      code: 'let style = { "--company": fill };\nconst a = <div style={style} />',
      errors: [{ messageId: 'inline' }],
    },
  ],
});

tester.run('segment-imports', rules['segment-imports'], {
  valid: [
    { code: "import { useUserListPage } from '../model/use-user-list-page';", filename: UI_FILE },
    { code: "import { toUserListView } from '../lib/user-list-view';", filename: MODEL_FILE },
    { code: "import type { SubmitEvent } from 'react';", filename: MODEL_FILE },
    { code: "import type { SubmitEvent } from 'react';", filename: LIB_FILE },
    { code: "import { useSyncExternalStore } from 'react';", filename: '/repo/src/shared/lib/store.ts' },
  ],
  invalid: [
    {
      code: "import { UserTable } from '../ui/UserTable';",
      filename: LIB_FILE,
      errors: [{ messageId: 'ui' }],
    },
    {
      code: "import type { UserTable } from '../ui/UserTable';",
      filename: LIB_FILE,
      errors: [{ messageId: 'ui' }],
    },
    {
      code: "import { UserTable } from '../ui/UserTable';",
      filename: MODEL_FILE,
      errors: [{ messageId: 'ui' }],
    },
    {
      code: "import { HelpTable } from '../ui/HelpTable';",
      filename: GROUP_LIB_FILE,
      errors: [{ messageId: 'ui' }],
    },
    {
      code: "import { UserTable } from '@/pages/user-list/ui/UserTable';",
      filename: MODEL_FILE,
      errors: [{ messageId: 'ui' }],
    },
    {
      code: "import { useState } from 'react';",
      filename: LIB_FILE,
      errors: [{ messageId: 'react' }],
    },
    {
      code: "import React from 'react';",
      filename: LIB_FILE,
      errors: [{ messageId: 'react' }],
    },
    {
      code: "import { useUserList } from '../model/use-user-list';",
      filename: LIB_FILE,
      errors: [{ messageId: 'hook' }],
    },
  ],
});

tester.run('no-index-key', rules['no-index-key'], {
  valid: [
    'const a = rows.map((row) => <li key={row.id}>{row.label}</li>)',
    'const a = rows.map((row, index) => <li key={row.id}>{index}</li>)',
    'const a = LABELS.map((label) => <li key={label}>{label}</li>)',
    'const a = <li key={pageIndex}>{label}</li>',
    'const a = rows.map((row, index) => <li key={row.id} data-index={index}>{row.label}</li>)',
  ],
  invalid: [
    {
      code: 'const a = rows.map((row, index) => <li key={index}>{row.label}</li>)',
      errors: [{ messageId: 'index' }],
    },
    {
      code: 'const a = rows.map((row, i) => <li key={`row-${i}`}>{row.label}</li>)',
      errors: [{ messageId: 'index' }],
    },
    {
      code: 'const a = rows.map((row, index) => <li key={index.toString()}>{row.label}</li>)',
      errors: [{ messageId: 'index' }],
    },
    {
      code: "const a = rows.map((row, index) => <li key={'row-' + index}>{row.label}</li>)",
      errors: [{ messageId: 'index' }],
    },
    {
      code: 'const a = rows.map((row, i) => <li key={String(i)}>{row.label}</li>)',
      errors: [{ messageId: 'index' }],
    },
    {
      code: 'const a = rows.flatMap((row, i) => <li key={String(i)}>{row.label}</li>)',
      errors: [{ messageId: 'index' }],
    },
  ],
});

const DETAILS_MODEL_FILE = '/repo/src/pages/user-details/model/use-user-details-page.ts';

tester.run('no-optional-id', rules['no-optional-id'], {
  valid: [
    { code: 'function useUserDetailsPage(userId: string) {}', filename: DETAILS_MODEL_FILE },
    { code: 'interface User { managerId: string | null }', filename: DETAILS_MODEL_FILE },
    { code: 'function isEmailTaken(email: string, exceptId?: string) {}', filename: '/repo/mocks/handlers.ts' },
    {
      code: 'function orderByOptionalIdQuery(orderId: string | undefined) {}',
      filename: '/repo/src/entities/order/model/order-queries.ts',
    },
    {
      code: 'function useUserDetailsPage(userId?: string) {}',
      filename: '/repo/src/pages/user-details/model/use-user-details-page.test.ts',
    },
    { code: "const label = role ?? ''", filename: DETAILS_MODEL_FILE },
  ],
  invalid: [
    {
      code: 'function useUserDetailsPage(userId?: string) {}',
      filename: DETAILS_MODEL_FILE,
      errors: [{ messageId: 'optional' }],
    },
    {
      code: 'function useUserDetailsPage(userId: string | undefined) {}',
      filename: DETAILS_MODEL_FILE,
      errors: [{ messageId: 'optional' }],
    },
    {
      code: "function useUserDetailsPage(id = '') {}",
      filename: DETAILS_MODEL_FILE,
      errors: [{ messageId: 'emptyDefault' }],
    },
    {
      code: "const id = props.userId ?? ''",
      filename: DETAILS_MODEL_FILE,
      errors: [{ messageId: 'emptyFallback' }],
    },
    {
      code: "const id = userId || ''",
      filename: DETAILS_MODEL_FILE,
      errors: [{ messageId: 'emptyFallback' }],
    },
    {
      code: 'function orderByOptionalIdQuery(orderId?: string) {}',
      filename: DETAILS_MODEL_FILE,
      errors: [{ messageId: 'optional' }],
    },
  ],
});

tester.run('no-raw-button', rules['no-raw-button'], {
  valid: [
    {
      code: 'const A = () => <button type="button">Save</button>',
      filename: '/repo/src/shared/ui/button/Button.tsx',
    },
    {
      code: 'const A = () => <button type="button">Save</button>',
      filename: '/repo/src/pages/user-list/ui/UserListPage.test.tsx',
    },
    { code: 'const A = () => <Button type="button">Save</Button>', filename: UI_FILE },
    { code: "const button = document.createElement('button')", filename: '/repo/e2e/probe.ts' },
  ],
  invalid: [
    {
      code: 'const A = () => <button type="button">Save</button>',
      filename: UI_FILE,
      errors: [{ messageId: 'raw' }],
    },
    {
      code: 'const A = () => <button type="button">Retry</button>',
      filename: '/repo/src/app/errors/AppErrorFallback.tsx',
      errors: [{ messageId: 'raw' }],
    },
  ],
});

describe('a project inside a folder named src', () => {
  const PROJECT = '/home/dev/src/app';
  const lint = (code, filename) =>
    new Linter({ cwd: PROJECT }).verify(
      code,
      {
        files: ['**/*.ts', '**/*.tsx'],
        languageOptions: { parser: tseslint.parser, parserOptions: { ecmaFeatures: { jsx: true } } },
        plugins: { architecture: { rules } },
        rules: {
          'architecture/no-optional-id': 'error',
          'architecture/no-raw-button': 'error',
          'architecture/no-cast-in-mapper': 'error',
        },
      },
      filename,
    );

  it('judges a file by its path from the lint root', () => {
    expect(lint('function isTaken(email: string, exceptId?: string) {}', `${PROJECT}/mocks/handlers.ts`)).toEqual([]);
    expect(lint('const A = () => <button type="button" />', `${PROJECT}/e2e/fixture.tsx`)).toEqual([]);
    expect(lint('const a = b as C', `${PROJECT}/scripts/build.ts`)).toEqual([]);
  });

  it('still reports the same code under the src of the project', () => {
    const ruleIds = (code, filename) => lint(code, filename).map((message) => message.ruleId);
    expect(ruleIds('function useX(userId?: string) {}', `${PROJECT}/src/pages/x/model/use-x.ts`)).toEqual([
      'architecture/no-optional-id',
    ]);
    expect(ruleIds('const A = () => <button type="button" />', `${PROJECT}/src/pages/x/ui/X.tsx`)).toEqual([
      'architecture/no-raw-button',
    ]);
  });
});

describe('rules coverage', () => {
  it('has a tester.run for every file in rules/', () => {
    const tests = readFileSync(new URL('./rules.test.js', import.meta.url), 'utf8');
    const names = readdirSync(new URL('./rules/', import.meta.url)).map((file) => file.replace(/\.js$/, ''));
    const untested = names.filter((name) => !tests.includes(`tester.run('${name}'`));
    expect(untested).toEqual([]);
  });
});
