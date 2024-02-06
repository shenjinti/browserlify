<script setup>
import { computed, ref, watch } from 'vue'
import { Listbox, ListboxButton, ListboxOption, ListboxOptions } from '@headlessui/vue'
import { CheckIcon, ChevronDownIcon } from '@heroicons/vue/20/solid'

const props = defineProps({
    options: {
        type: Array,
        required: true,
    },
    value: {
        type: [Object, Number, String],
    },
    placeholder: {
        type: [String, Number],
        default: 'select option',
    },
    disabled: {
        type: Boolean,
        default: false,
    },
    height: {
        type: String,
        default: 'py-2',
    },
    icon: {
        type: Function,
        default: () => { },
    },
})

defineEmits(['update:value'])

const select = ref(props.value)
const label = computed(() => (val) => {
    if (val.label)
        return val.label
    const item = props.options.find(e => e.value === val)
    return item?.label || item?.value || val
})

watch(() => props.value, val => select.value = val)
</script>

<template>
    <div class="w-full">
        <div ref="el" />
        <Listbox v-model="select" :disabled="disabled" as="div" @update:model-value="$emit('update:value', select.value)">
            <div class="relative">
                <ListboxButton class="relative w-full cursor-default rounded-md
          border border-gray-300 bg-white pl-3 pr-10 text-left
        focus:border-secondary focus:outline-none ring-transparent text-xs"
                    :class="select ? 'py-[7px]' : 'py-2'">
                    <template v-if="select">
                        <span class="block truncate font-medium text-sm">
                            {{ label(select) }}
                        </span>
                    </template>
                    <template v-else>
                        <span class="block truncate text-xs text-gray-400">
                            {{ placeholder }}
                        </span>
                    </template>
                    <span class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2">
                        <ChevronDownIcon class="h-5 w-5 text-gray-400" aria-hidden="true" />
                    </span>
                </ListboxButton>
                <transition leave-active-class="transition ease-in duration-100" leave-from-class="opacity-100"
                    leave-to-class="opacity-0">
                    <ListboxOptions class="absolute z-10 mt-1 max-h-60 w-full overflow-auto
            rounded-md bg-white py-1 text-base shadow-lg ring-1 ring-black ring-opacity-5
            focus:outline-none sm:text-sm">
                        <ListboxOption v-for="option in options" :key="option.value" v-slot="{ active, selected }"
                            :value="option" :disabled="option.value === select" as="template">
                            <li class="relative cursor-default select-none py-2 pl-3 pr-9"
                                :class="[active || select === option.value ? 'text-white bg-primary' : 'text-gray-900']">
                                <div class="flex items-center">
                                    <!-- <img v-if="icon" :src="option.icon" alt="" class="h-5 w-5 flex-shrink-0 rounded-full"> -->
                                    <slot name="prefix" :option="option" />
                                    <span class="block truncate"
                                        :class="[selected || select === option.value ? 'font-semibold' : 'font-normal']">
                                        {{ option.label }}
                                    </span>
                                    <slot name="suffix" :option="option" />
                                </div>

                                <span v-if="selected || select === option.value"
                                    class="absolute  inset-y-0 right-0 flex items-center pr-4"
                                    :class="[active || select === option.value ? 'text-white' : 'text-primary']">
                                    <CheckIcon class="h-5 w-5" aria-hidden="true" />
                                </span>
                            </li>
                        </ListboxOption>
                    </ListboxOptions>
                </transition>
            </div>
        </Listbox>
    </div>
</template>
