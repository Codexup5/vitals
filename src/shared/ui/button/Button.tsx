import type { ComponentPropsWithRef } from 'react';

type ButtonProps = Omit<ComponentPropsWithRef<'button'>, 'type'> & {
  type: 'button' | 'submit' | 'reset';
};

export const Button = ({ type, ...props }: ButtonProps) => <button type={type} {...props} />;
