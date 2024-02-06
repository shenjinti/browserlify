<script setup>
import { computed, ref, watch } from 'vue'

const props = defineProps({
  value: {
    type: String,
    required: true,
  },
  placeholder: {
    type: String,
    default: 'Type here...',
  },
  type: {
    type: String,
    default: 'text',
  },
  label: {
    type: String,
    default: '',
  },
  pointerEvents: {
    type: String,
    default: 'pointer-events-none',
  },
  disabled: {
    type: Boolean,
    default: false,
  },
  required: {
    type: Boolean,
    default: false,
  },
  /**
   * validate: () => true,
   * message: 'error',
   * time: 0,
   */
  rule: {
    type: Object,
    default: () => { },
  },
})

const emits = defineEmits(['update:value', 'onKeyUpEnter'])

const value = ref(props.value)
watch(() => props.value, val => value.value = val)

const showErr = ref(false)

function validate(fn) {
  if (props.rule) {
    if (props.rule.validate()) {
      fn(null)
      showErr.value = false
    }
    else {
      fn(true)
      showErr.value = true
      if (props.rule.time) {
        setTimeout(() => {
          showErr.value = false
        }, props.rule.time)
      }
    }
  }
}

function onUpdateValue() {
  emits('update:value', value.value)
  validate(() => { })
}

const errClass = computed(() => showErr.value && 'border border-red-500')
defineExpose({ validate })
</script>

<template>
  <div class="w-full">
    <div class="relative">
      <div class="relative rounded-md">
        <div v-if="$slots.prefix" class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
          <slot name="prefix" />
        </div>
        <div>
          <input ref="inputRef" v-model="value" :type="type" :placeholder="placeholder" :disabled="disabled" class="block w-full px-2 rounded-md border border-gray-300 focus:border-primary  py-1.5 text-sm focus:ring-1 ring-indigo-600 placeholder-gray-400 placeholder:text-xs
           focus:bg-white focus:text-gray-900 focus:placeholder-gray-500 focus:outline-none
            focus:border-secondary focus:ring-secondary sm:text-sm"
            @keyup.enter="$emit('onKeyUpEnter')" @change="onUpdateValue" @blur="onUpdateValue">
        </div>
      </div>
    </div>
  </div>
</template>
