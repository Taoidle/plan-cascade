import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { WorkflowCardRenderer } from './WorkflowCardRenderer';

describe('WorkflowCardRenderer', () => {
  it('renders a visible warning card for unknown card types', () => {
    render(
      <WorkflowCardRenderer
        payload={
          {
            cardType: 'unknown_card_type',
            cardId: 'card-42',
            data: {},
            interactive: false,
          } as never
        }
      />,
    );

    expect(screen.getByText('Unknown workflow card')).toBeTruthy();
    expect(screen.getByText(/unknown_card_type/)).toBeTruthy();
    expect(screen.getByText(/card-42/)).toBeTruthy();
  });
});
