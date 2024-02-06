<script setup>
import { ref, computed, onMounted } from 'vue'
import { XMarkIcon } from '@heroicons/vue/24/solid'
const props = defineProps({
    modelValue: {
        type: Boolean,
        default: false,
    },
    autoClose: {
        type: Boolean,
        default: true,
    },
})
const emit = defineEmits({
    'update:modelValue': (value) => true,
    'close': () => true,
})

const isOpen = computed({
    get: () => props.modelValue,
    set: val => emit('update:modelValue', val),
})

function close() {
    isOpen.value = false
    emit('close')
}

</script>
<template>
    <div v-if="isOpen"
        class="mask min-h-screen fixed top-0 left-0 right-0 z-50 bottom-0 bg-gray-900 w-full h-full opacity-100">
        <div class="w-full h-full relative justify-center flex items-center">
            <div v-if="isOpen"
                class="modal relative space-y-3 z-50 bg-white shadow-md max-w-lg rounded-md p-6">
                    <XMarkIcon v-if="autoClose" class="w-6 h-6 absolute right-2 top-2 text-gray-600 cursor-pointer hover:text-gray-400" @click="close" />
                <slot></slot>
               
            </div>
        </div>
    </div>
</template>

<style scoped>
.modal {
    position: fixed;
    z-index: 999999;
}

.mask {
    position: fixed;
    z-index: 9999;
    left: 0;
    right: 0;
    top: 0;
    bottom: 0;
    background: rgba(0, 0, 0, .5);
}
</style>