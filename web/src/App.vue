<script setup>
import RFB from '@novnc/novnc/core/rfb.js';
import { ref } from 'vue';

const status = ref('Loading');
const desktopName = ref('');

function connectedToServer(e) {
  status.value = "Connected";
}

function disconnectedFromServer(e) {
  if (e.detail.clean) {
    status.value = "Disconnected";
  } else {
    status.value = "Something went wrong, connection is closed";
  }
}

function updateDesktopName(e) {
  desktopName.value = e.detail.name;
}

async function doConnect() {
  let url = 'ws://192.168.3.104:9000/remote/connect/unittest'
  let rfb = new RFB(document.getElementById('screen'), url,)

  // Add listeners to important events from the RFB module
  rfb.addEventListener("connect", connectedToServer);
  rfb.addEventListener("disconnect", disconnectedFromServer);
  rfb.addEventListener("desktopname", updateDesktopName);
  // Set parameters that can be changed on an active connection
}
</script>

<template>
  <div>
    <div id="top_bar">
      <div>{{ status }}</div>
      <div>{{ desktopName }}</div>
      <button class="rounded bg-gray-500" @click="doConnect"> connect</button>
    </div>
    <div class="">
      <div id="screen" class="flex bg-gray-200 w-full h-screen">
      </div>
    </div>
  </div>
</template>