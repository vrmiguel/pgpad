<script lang="ts" module>
	import { cn, type WithElementRef } from '$lib/utils.js';
	import type { HTMLAnchorAttributes, HTMLButtonAttributes } from 'svelte/elements';
	import { type VariantProps, tv } from 'tailwind-variants';

	export const buttonVariants = tv({
		base: "focus-ring animate-press inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-lg text-sm font-medium outline-none transition-all duration-200 disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 [&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		variants: {
			variant: {
				default:
					'bg-primary text-primary-foreground shadow-md hover:bg-primary-dark hover:shadow-lg active:scale-[0.98]',
				destructive:
					'bg-error text-error-foreground shadow-md hover:bg-red-600 hover:shadow-lg focus-visible:ring-error/30',
				outline:
					'border border-border bg-background hover:bg-accent hover:text-accent-foreground shadow-sm hover:shadow-md hover:border-border-light',
				secondary:
					'bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80 hover:shadow-md',
				ghost: 'hover:bg-accent hover:text-accent-foreground hover:shadow-sm',
				link: 'text-primary underline-offset-4 hover:underline hover:text-primary-dark',
				success:
					'bg-success text-success-foreground shadow-md hover:bg-green-600 hover:shadow-lg focus-visible:ring-success/30',
				warning:
					'bg-warning text-warning-foreground shadow-md hover:bg-amber-600 hover:shadow-lg focus-visible:ring-warning/30'
			},
			size: {
				default: 'h-10 px-4 py-2 has-[>svg]:px-3',
				sm: 'h-8 gap-1.5 rounded-md px-3 text-xs has-[>svg]:px-2.5',
				lg: 'h-12 rounded-lg px-6 text-base has-[>svg]:px-5',
				icon: 'size-10 rounded-lg',
				'icon-sm': 'size-8 rounded-md',
				'icon-lg': 'size-12 rounded-lg'
			}
		},
		defaultVariants: {
			variant: 'default',
			size: 'default'
		}
	});

	export type ButtonVariant = VariantProps<typeof buttonVariants>['variant'];
	export type ButtonSize = VariantProps<typeof buttonVariants>['size'];

	export type ButtonProps = WithElementRef<HTMLButtonAttributes> &
		WithElementRef<HTMLAnchorAttributes> & {
			variant?: ButtonVariant;
			size?: ButtonSize;
		};
</script>

<script lang="ts">
  let {
    class: className,
    variant = 'default',
    size = 'default',
    ref = $bindable(null),
    href = undefined,
    type = 'button',
    disabled,
    children,
    onclick,
    ondblclick,
    oncontextmenu,
    ...restProps
  }: ButtonProps = $props();
</script>

{#if href}
  <a
    bind:this={ref}
    data-slot="button"
    class={cn(buttonVariants({ variant, size }), className)}
    href={disabled ? undefined : href}
    aria-disabled={disabled}
    role={disabled ? 'link' : undefined}
    tabindex={disabled ? -1 : undefined}
    onclick={onclick}
    ondblclick={ondblclick}
    oncontextmenu={oncontextmenu}
    {...restProps}
  >
    {@render children?.()}
  </a>
{:else}
  <button
    bind:this={ref}
    data-slot="button"
    class={cn(buttonVariants({ variant, size }), className)}
    type={type}
    disabled={disabled}
    onclick={onclick}
    ondblclick={ondblclick}
    oncontextmenu={oncontextmenu}
    {...restProps}
  >
    {@render children?.()}
  </button>
{/if}
