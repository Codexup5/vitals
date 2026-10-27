import { z } from 'zod';

import { reportError } from '../lib/report-error';
import { ApiError } from './api-error';

const MAX_REPORTED_SIGNATURES = 100;

const reportedSignatures = new Set<string>();

const markReported = (signature: string) => {
  if (reportedSignatures.has(signature)) return false;
  if (reportedSignatures.size >= MAX_REPORTED_SIGNATURES) reportedSignatures.clear();
  reportedSignatures.add(signature);
  return true;
};

export const lenientArray = <Item extends z.ZodType>(item: Item, source: string) =>
  z.array(z.unknown()).transform((values) => {
    const items: z.output<Item>[] = [];
    const droppedIndexes: number[] = [];
    let firstError: z.ZodError | undefined;
    values.forEach((value, index) => {
      const result = item.safeParse(value);
      if (result.success) {
        items.push(result.data);
        return;
      }
      droppedIndexes.push(index);
      firstError ??= result.error;
    });

    if (firstError) {
      const signature = JSON.stringify([
        source,
        values.length,
        droppedIndexes,
        firstError.issues.map((issue) => [issue.path, issue.code]),
      ]);
      if (!markReported(signature)) return items;
      reportError(
        new ApiError({
          kind: 'schema',
          status: 200,
          code: 'SCHEMA_MISMATCH',
          message: `${source}: ${droppedIndexes.length} of ${values.length} list items do not match schema`,
          cause: firstError,
        }),
        { source, droppedIndexes, issues: firstError.issues },
      );
    }
    return items;
  });
