import { describe, expect, it } from 'vitest';

import { registerAdversarialTests } from '../../../tests/adversarial/frontend-cases';

describe('C0.1 adversarial regressions (intentionally red until the builder fixes them)', () => {
  registerAdversarialTests({ it, expect });
});
