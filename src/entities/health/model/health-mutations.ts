import { useMutation } from '@tanstack/react-query';

import { revealMinidump } from '../api/health-api';

/** Opens Explorer on the dump; nothing in the cache changes. */
export const useRevealMinidump = () =>
  useMutation({
    mutationFn: (name: string) => revealMinidump(name),
  });
