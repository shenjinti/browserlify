<script setup>
import { ref, computed, onMounted } from 'vue'
import { PlayIcon, StopIcon, TrashIcon, PhotoIcon, ArrowsPointingOutIcon, ArrowUpTrayIcon } from '@heroicons/vue/24/outline'
import Button from '../src/compotents/Button.vue'
import Confirm from '../src/compotents/Confirm.vue'

const isOpen = ref(false)
const showWindow = ref(false)
const confirmVisible = ref(false)
const deletecontent = ref('是否确认删除？')

//打开窗口，并且showWindow为true
async function onOpenWindow(item) {
  showWindow.value = true
  console.log(item)
}

async function handleUpload() {
  console.log('upload')
}

async function handlePhoto(item) {
  console.log('photo', item)
}

function handlePause(item) {
  // item.pause = !item.pause
  console.log('pause', item)
}

async function onDelete(item) {
  confirmVisible.value = true

  console.log('delete', item)
}

async function handleDelete() {
  confirmVisible.value = false
  deletecontent.value = '删除后将无法恢复，是否确认删除?'
  console.log('confirmDelete')
}

async function handleEnLarge(item) {
  console.log('open', item)
}

// 放大
async function handleMax(item) {
  console.log('max', item)
}
</script>

<template>
  <div class="max-w-7xl mx-auto py-10">
    <div v-if="showWindow">
      <div class="flex items-center justify-between border-b pb-2">
        <div class="flex items-center space-x-3">
          <div class="cursor-pointer hover:text-blue-500 mr-6" @click="showWindow = false">back</div>
          <img src="../public/chrome.jpg" alt="" class="w-7 h-7">
          <div class="flex items-center justify-between py-1 bg-gray-100 rounded-lg px-3">
            <p>www1(192.168.0.1)</p>
          </div>
        </div>

        <div class="flex items-center space-x-4">
          <ArrowUpTrayIcon class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer" @click="handleUpload()" />
          <PhotoIcon class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer" @click="handlePhoto('a')" />
          <ArrowsPointingOutIcon class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer"
            @click="handleEnLarge('a')" />
          <PlayIcon class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer" @click="handlePause('a')" />
          <StopIcon class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer" @click="handlePause('a')" />
          <TrashIcon class="w-6 h-6 text-gray-600 hover:text-red-600 cursor-pointer" @click="onDelete('a')" />
        </div>
      </div>
      <div class="flex min-h-[40rem] my-6 bg-[#040a0f] w-full rounded-md">

      </div>
    </div>
    <div v-else>
      <div class="flex items-center justify-between">
        <Button>Create Browser</Button>
        <div class="flex space-x-4">
          <Button>Headless</Button>
          <Button>Content API</Button>
        </div>
      </div>

      <div class="grid grid-cols-3 gap-6 mt-10">
        <div v-for="(item, index) in 3" class="shadow bg-white rounded pt-2 group">
          <div class="flex w-full justify-end space-x-3 px-5">

            <StopIcon v-if="item.pause" class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer"
              @click="handlePause(item)" />
            <PlayIcon v-else class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer" @click="handlePause(item)" />

            <TrashIcon class="w-6 h-6 text-gray-600 hover:text-red-600 cursor-pointer" @click="onDelete(item)" />
          </div>
          <div class="h-60 w-60 mx-auto cursor-pointer my-2" @click="onOpenWindow(item)">
            <img src="../public/chrome.jpg" alt="" class="w-full h-full object-cover">
          </div>
          <div class="flex items-center justify-between py-2 px-4 bg-gray-100 invisible group-hover:visible">
            <p>www1(192.168.0.1)</p>
            <img src="../public/chrome.jpg" alt="" class="w-6 h-6">
          </div>
        </div>
      </div>
    </div>
    <Confirm v-model:open="confirmVisible" @positive-click="handleDelete()" title='确认删除' :content=deletecontent
      width-class="max-w-lg">
    </Confirm>
  </div>
</template>

