const CLASS_CASES = {
  kebab: {
    pattern: '^[a-z][a-z0-9]*(-[a-z0-9]+)*$',
    message: 'A CSS-module class is kebab-case: `.card-header` → `styles.cardHeader`.',
  },
  camel: {
    pattern: '^[a-z][a-zA-Z0-9]*$',
    message: 'A CSS-module class is camelCase: .cardHeader → styles.cardHeader.',
  },
};

export const classNameRule = (classCase) => {
  if (!Object.hasOwn(CLASS_CASES, classCase)) {
    throw new Error(`Unknown class case, use ${Object.keys(CLASS_CASES).join(' or ')}`, { cause: classCase });
  }
  const { pattern, message } = CLASS_CASES[classCase];
  return [pattern, { message, reportDisables: true }];
};
