<script setup>
defineProps({
    type: { type: String, default: 'default' },
    size: { type: String, default: 'base', validator: val => ['sm', 'base', 'lg'].includes(val) },
    disabled: { type: Boolean, default: false },
})

const colors = {
    default: 'text-gray-700 bg-white border border-gray-400 hover:bg-gray-50 focus:ring-gray-400',
    success: 'text-white bg-green-500 hover:bg-green-700 focus:ring-green-500',
    info: 'text-white bg-blue-500 hover:bg-blue-700 focus:ring-blue-500',
    warning: 'text-white bg-yellow-500 hover:bg-yellow-700 focus:ring-yellow-500',
    error: 'text-white bg-red-500 hover:bg-red-700 focus:ring-red-500',
    primary: 'text-white bg-primary hover:bg-opacity-80 focus:ring-primary',
    secondary: 'text-white bg-secondary hover:bg-opacity-80 focus:ring-secondary',
}
</script>

<template>
    <button :disabled="disabled" type="button" class="inline-flex w-full items-center justify-center whitespace-nowrap
    rounded-md font-sans text-xs font-semibold leading-4 shadow-sm
    focus:outline-none focus:ring-1 focus:ring-offset-1
    disabled:cursor-not-allowed disabled:opacity-30
    sm:w-auto" :class="[colors[type], {
        'px-2 py-1': size === 'sm',
        'px-3 py-2.5 sm:py-2': size === 'base',
        'px-5 py-3.5': size === 'lg',
    }]">
        <slot />
    </button>
</template>
