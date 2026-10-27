import lazyQueryFn from './rules/lazy-query-fn.js';
import noCastInMapper from './rules/no-cast-in-mapper.js';
import noDtoOutsideApi from './rules/no-dto-outside-api.js';
import noIndexKey from './rules/no-index-key.js';
import noInlineQueryKey from './rules/no-inline-query-key.js';
import noInlineStyle from './rules/no-inline-style.js';
import noJunkComments from './rules/no-junk-comments.js';
import noLogicInUi from './rules/no-logic-in-ui.js';
import noOptionalId from './rules/no-optional-id.js';
import noRawButton from './rules/no-raw-button.js';
import noSwallowedError from './rules/no-swallowed-error.js';
import queryOptionsInEntity from './rules/query-options-in-entity.js';
import segmentImports from './rules/segment-imports.js';

export const rules = {
  'lazy-query-fn': lazyQueryFn,
  'no-cast-in-mapper': noCastInMapper,
  'no-dto-outside-api': noDtoOutsideApi,
  'no-index-key': noIndexKey,
  'no-inline-query-key': noInlineQueryKey,
  'no-inline-style': noInlineStyle,
  'no-junk-comments': noJunkComments,
  'no-logic-in-ui': noLogicInUi,
  'no-optional-id': noOptionalId,
  'no-raw-button': noRawButton,
  'no-swallowed-error': noSwallowedError,
  'query-options-in-entity': queryOptionsInEntity,
  'segment-imports': segmentImports,
};

const plugin = { meta: { name: 'eslint-plugin-architecture', version: '1.0.0' }, rules };

export const recommended = Object.fromEntries(Object.keys(rules).map((name) => [`architecture/${name}`, 'error']));

export default plugin;
