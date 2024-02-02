<script setup>
import { ref, onMounted } from 'vue'
import { PlayIcon, StopIcon, TrashIcon, PhotoIcon, ArrowsPointingOutIcon, ArrowUpTrayIcon, ArrowLongLeftIcon, PencilSquareIcon } from '@heroicons/vue/24/outline'
import Button from '../src/compontents/Button.vue'
import Confirm from '../src/compontents/Confirm.vue'
// import Input from '../src/compontents/Input.vue'
import RFB from '\@novnc/novnc/core/rfb.js';

const loading = ref(false)
const showEdit = ref(false)
const confirmVisible = ref(false)
const deletecontent = ref('Are you sure you want to delete?')
const remotes = ref([])
const current = ref(null)
const confirmDeleteId = ref(null)
const rfbScreen = ref(null)

onMounted(async () => {
  await loadRemotes()
})


function showTip(text, e) {
  loading.value = false
  console.log(text, e)
}

async function loadRemotes() {
  try {
    let resp = await fetch('/remote/list', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      }
    })
    let items = await resp.json() || []
    items.forEach(item => {
      item.screenshot = undefined;
      item.title = item.name || item.id
      if (item.running) {
        item.screenshot = `/screen/${item.id}?t=${Date.now()}`
      }
    })
    remotes.value = items
  } catch (e) {
    showTip('Failed to load remotes', e)
  }
}

async function stopRemote(id) {
  try {
    await fetch(`/remote/stop/${id}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({})
    })
  } catch (e) {
    showTip('Failed to stop remote', e)
  }
}
async function startRemote(id) {
  try {
    await fetch(`/remote/start/${id}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({})
    })
  } catch (e) {
    showTip('Failed to start remote', e)
  }
}

async function deleteRemote(id) {
  try {
    await fetch(`/remote/remove/${id}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({})
    })
  } catch (e) {
    showTip('Failed to delete remote', e)
  }
}

async function editRemote(item, data) {
  try {
    let resp = await fetch(`/remote/edit/${item.id}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(data)
    })

    let { name, http_proxy, homepage } = await resp.json();
    item.name = name
    item.http_proxy = http_proxy
    item.homepage = homepage
    item.title = item.name || item.id
  } catch (e) {
    showTip('Failed to edit remote', e)
  }
}

async function doCreateRemote() {
  loading.value = true
  try {
    await fetch('/remote/create', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({})
    })
    await loadRemotes()
  } catch (e) {
    showTip('Failed to create remote', e)
  }
  loading.value = false
}

async function onOpenWindow(item) {
  current.value = item
  tryConnectRemote(item)
}

async function onGoback() {
  if (current.value.rfb) {
    current.value.rfb.disconnect()
    current.value.rfb = undefined
  }

  current.value = undefined
  await loadRemotes()
}

async function handleUpload() {
  console.log('upload')
}

async function handleScreenshot(item) {
  if (item.running) {
    let a = document.createElement('a')
    a.href = `/screen/${item.id}`
    a.download = 'screenshot.png'
    a.click()
  }
}

async function handleStop(item) {
  loading.value = true
  await stopRemote(item.id)
  item.screenshot = undefined
  loading.value = false
  item.running = false

  if (item.rfb) {
    item.rfb.disconnect()
    item.rfb = undefined
  }
}

async function handleStart(item) {
  loading.value = true
  await startRemote(item.id)
  loading.value = false
  item.running = true
  setTimeout(() => {
    if (item.running) {
      item.screenshot = `/screen/${item.id}`
    }
  }, 1000)
}

async function handleStartAndConnect(item) {
  loading.value = true
  await startRemote(item.id)
  loading.value = false
  item.running = true

  setTimeout(() => {
    tryConnectRemote(item)
  }, 100)
}

const tryConnectTimerId = ref(null)

function tryConnectRemote(item) {
  if (tryConnectTimerId.value) {
    return;
  }

  tryConnectTimerId.value = setInterval(() => {
    connectRemote(current.value)
  }, 50)
}

function connectRemote(item) {
  let target = document.getElementById('rfb-screen');
  if (!target) {
    return;
  }

  if (item.rfb) {
    item.rfb.disconnect()
    item.rfb = undefined
  }

  clearInterval(tryConnectTimerId.value)
  tryConnectTimerId.value = null

  let scheme = window.location.protocol === 'https:' ? 'wss' : 'ws'
  let url = `${scheme}://${window.location.host}/remote/connect/${item.id}`
  let rfb = new RFB(target, url)

  rfb.addEventListener("connect", (e) => {
    rfb.scaleViewport = true
  });

  rfb.addEventListener("disconnect", (e) => {
    item.rfb = undefined
    item.screenshot = undefined
  });

  item.rfb = rfb
}

async function onShowConfirmDelete(item) {
  confirmVisible.value = true
  confirmDeleteId.value = item.id
}

async function handleDelete() {
  confirmVisible.value = false
  let id = confirmDeleteId.value
  confirmDeleteId.value = null
  if (id === null) {
    return
  }

  if (current.value && current.value.id === id) {
    current.value = null
  }

  loading.value = true
  await deleteRemote(id)
  loading.value = false
  remotes.value = remotes.value.filter(item => item.id !== id)
}

async function handleFullscreen(item) {
  console.log('handleFullscreen', item)
}

async function handleTitlechange() {
  await editRemote(current.value, { name: current.value.title })
}

</script>

<template>
  <div class="max-w-7xl mx-auto py-7">
    <div class="mb-5 w-36">
      <a href="https://browserlify.com/" target="_blank" class="flex items-center space-x-2 w-auto">
        <img src="../public/logo.png" alt="" class="w-7 h-7">
        <span class="font-semibold text-lg text-gray-700">Browserlify</span>
      </a>
    </div>


    <div v-if="current">
      <div class="flex items-center justify-between border-b pb-2">
        <div class="flex items-center space-x-3 w-full">
          <div class="cursor-pointer group flex mr-6" @click="onGoback()">
            <ArrowLongLeftIcon class="w-6 h-6 group-hover:text-sky-600" />
            <p class="group-hover:text-sky-600 font-semibold ml-2">Back</p>
          </div>
          <img src="../public/chrome.png" alt="" class="w-7 h-7">
          <div class="flex items-center space-x-2 w-96">
            <input v-if="showEdit" id="titleInput" ref="titleInput" v-model="current.title" type="text"
              :placeholder="current.title" class="block w-full h-9 rounded-md border border-gray-300 py-1.5 pl-2 text-sm placeholder-gray-400 placeholder:text-xs
                      focus:bg-white focus:text-gray-900 focus:placeholder-gray-500 focus:outline-none
                      focus:border-secondary focus:ring-gray-200 sm:text-sm"
              @keyup.enter="showEdit = false, handleTitlechange()" @change="handleTitlechange()"
              @blur="showEdit = false, handleTitlechange()">
            <div v-else class="flex justify-center items-center h-9">
              <p>{{ current.title }}</p>
            </div>

            <PencilSquareIcon v-if="!showEdit" class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer"
              @click="showEdit = true" />
          </div>
        </div>

        <div class="flex items-center space-x-4">
          <!-- <ArrowUpTrayIcon class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer"
            @click="handleUpload(current)" /> -->
          <PhotoIcon class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer" @click="handleScreenshot(current)" />
          <!-- <ArrowsPointingOutIcon class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer"
            @click="handleFullscreen(current)" /> -->
          <PlayIcon v-if="!current.running" class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer"
            @click="handleStartAndConnect(current)" />
          <StopIcon v-else class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer" @click="handleStop(current)" />
          <TrashIcon class="w-6 h-6 text-gray-600 hover:text-red-600 cursor-pointer"
            @click="onShowConfirmDelete(current)" />
        </div>
      </div>
      <div v-if="current.running">
        <div id="rfb-screen" class="flex min-h-[40rem] my-6 bg-[#040a0f] w-full rounded-md">
        </div>
      </div>
      <div v-else="current.running" class="h-96 w-96 mx-auto mt-10">
        <div class="rounded flex items-center justify-center w-full h-full text-white bg-gray-700">
          <h1>Browser is shutdown</h1>
        </div>
      </div>
    </div>
    <div v-else>
      <div class="flex items-center justify-between">
        <div class="flex items-center space-x-6">

          <div class="flex items-center space-x-2">
            <Button @click="doCreateRemote"> Create Browser </Button>
            <img v-if="loading" src="../public/loading.png" alt="" class="w-5 h-5 animate-spin">
          </div>


        </div>
        <div class="flex space-x-6">
          <a href="#" class="hover:underline">Headless</a>
          <a href="#" class="hover:underline">Content API</a>
        </div>
      </div>
      <template v-if="remotes.length > 0">
        <div class="grid grid-cols-3 gap-6 mt-10">
          <div v-for="item in remotes" class="shadow bg-white rounded pt-2 group hover:shadow-lg" :key="item.id">

            <div class="flex w-full justify-end space-x-3 px-5">
              <StopIcon v-if="item.running"
                class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer invisible group-hover:visible"
                @click="handleStop(item)" />
              <PlayIcon v-else
                class="w-6 h-6 text-gray-600 hover:text-sky-600 cursor-pointer invisible group-hover:visible"
                @click="handleStart(item)" />

              <TrashIcon class="w-6 h-6 text-gray-600 hover:text-red-600 cursor-pointer invisible group-hover:visible"
                @click="onShowConfirmDelete(item)" />
            </div>
            <div class="min-h-80 bg-gray-50 mx-auto cursor-pointer flex items-center justify-center"
              @click="onOpenWindow(item)">
              <div v-if="item.running">
                <img v-show="item.screenshot" :src="item.screenshot" alt="" class="w-full h-full">
              </div>
              <div v-else class="flex items-center justify-center w-full h-full">
                <h1>Browser is shutdown</h1>
              </div>
            </div>
            <div class="flex items-center justify-between py-2 px-4 bg-slate-200">
              <p>{{ item.title }}</p>
              <img src="../public/chrome.png" alt="" class="w-6 h-6">
            </div>
          </div>
        </div>
      </template>
      <div v-else class="text-xl flex justify-center items-center pt-16 text-gray-600">
        Create a remote browser first
      </div>
    </div>
    <Confirm v-model:open="confirmVisible" @positive-click="handleDelete()" title='Confirm delete' :content=deletecontent
      width-class="max-w-lg">
    </Confirm>
  </div>
</template>

