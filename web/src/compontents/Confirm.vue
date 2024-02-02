<script setup>
import { ref, computed } from 'vue'
import {
  Dialog,
  DialogPanel,
  TransitionChild,
  TransitionRoot,
} from '@headlessui/vue'

import { ExclamationTriangleIcon, XMarkIcon } from '@heroicons/vue/24/outline'
import TButton from './Button.vue'

const props = defineProps({
  title: { type: String },
  content: { type: String },
  open: { type: Boolean, default: true },
  showIcon: { type: Boolean, default: true },
  positiveText: { type: String, default: 'OK' },
  negativeText: { type: String, default: 'CANCEL' },
  onPositiveClick: { type: Function, default: () => { } },
  onNegativeClick: { type: Function, default: () => { } },
  onClose: { type: Function, default: () => { } },
  widthClass: { type: String, default: 'max-w-2xl' },
})

// const show = ref(props.open)

function closeModal(action) {
  show.value = false
  setTimeout(() => {
    action ? props.onPositiveClick() : props.onNegativeClick()
    props.onClose(action)
  }, 300)
}

const emit = defineEmits(['update:open'])
const show = computed({
  get() {
    return props.open
  },
  set(val) {
    emit('update:open', val)
  }
})
</script>

<template>
  <TransitionRoot appear :show="show" as="template">
    <Dialog as="div" class="relative z-10" @close="closeModal(false)">
      <TransitionChild as="template" enter="duration-300 ease-out" enter-from="opacity-0" enter-to="opacity-100"
        leave="duration-200 ease-in" leave-from="opacity-100" leave-to="opacity-0">
        <div class="fixed z-0 inset-0 bg-black bg-opacity-25" />
      </TransitionChild>

      <div class="fixed inset-0 overflow-y-auto">
        <div class="flex min-h-full items-center justify-center p-4 text-center ">
          <TransitionChild as="template" enter="ease-out duration-300"
            enter-from="opacity-0 translate-y-4 sm:translate-y-0 sm:scale-95"
            enter-to="opacity-100 translate-y-0 sm:scale-100" leave="ease-in duration-200"
            leave-from="opacity-100 translate-y-0 sm:scale-100"
            leave-to="opacity-0 translate-y-4 sm:translate-y-0 sm:scale-95">
            <DialogPanel class="relative px-4 pt-5 pb-4 transform overflow-hidden
              rounded-lg bg-white text-left shadow-xl transition-all
              sm:my-8 w-full sm:p-6" :class="widthClass">
              <div class="absolute top-0 right-0 hidden pt-4 pr-4 sm:block">
                <button type="button" class="rounded-md bg-white text-gray-400
                  hover:text-gray-500 focus:outline-none focus:ring-2 focus:ring-secondary focus:ring-offset-2"
                  @click="closeModal(false)">
                  <span class="sr-only">Close</span>
                  <XMarkIcon class="h-6 w-6" aria-hidden="true" />
                </button>
              </div>

              <div class="flex items-start">
                <div class="flex-shrink-0">
                  <div v-if="showIcon"
                    class="mx-auto flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-full bg-red-200 sm:mx-0 sm:h-10 sm:w-10">
                    <ExclamationTriangleIcon class="h-6 w-6 text-red-600" aria-hidden="true" />
                  </div>
                </div>
                <div class="ml-3 w-0 flex-1 pt-0.5">
                  <p class="text-lg font-bold text-gray-900">
                    {{ title }}
                  </p>
                  <div class="mt-3 text-sm text-gray-700 whitespace-wrap tracking-widest">
                    <div v-html="content" />
                  </div>
                  <div class="mt-5 sm:mt-6 space-y-3 sm:space-y-0 sm:space-x-7 text-right">
                    <TButton type="error" @click="closeModal(true)">
                      {{ positiveText }}
                    </TButton>
                    <TButton @click="closeModal(false)">
                      {{ negativeText }}
                    </TButton>
                  </div>
                </div>
                <div class="ml-4 flex flex-shrink-0">
                  <button type="button" class="inline-flex rounded-md bg-white text-gray-400 hover:text-gray-500
                  focus:outline-none focus:ring-2 focus:ring-secondary focus:ring-offset-2" @click="show = false">
                    <span class="sr-only">Close</span>
                    <XMarkIcon class="h-5 w-5" aria-hidden="true" />
                  </button>
                </div>
              </div>
            </DialogPanel>
          </TransitionChild>
        </div>
      </div>
    </Dialog>
  </TransitionRoot>
</template>
