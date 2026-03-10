import { render, screen } from '@testing-library/react';

test('renders login page', () => {
  render(<LoginPage />);
  expect(screen.getByText('Welcome')).toBeInTheDocument();
});

test('renders submit button', () => {
  render(<Form />);
  const button = screen.getByRole('button', { name: 'Submit' });
  expect(button).toBeVisible();
});
