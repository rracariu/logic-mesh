import { createApp } from 'vue'
import App from './App.vue'
import ToastService from 'primevue/toastservice'
import PrimeVue from 'primevue/config'

const app = createApp(App)
app.use(PrimeVue)
app.use(ToastService)
app.mount('#app')
