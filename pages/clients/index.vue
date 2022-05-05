<template>
    <div>

        <part>
            <nuxt-content :document="page" />

            <table cellpadding="0" cellspacing="0" border="0">
                <thead>
                    <tr>
                        <th>Sl</th>
                        <th class="u-dt">Title</th>
                        <th class="u-dt">Sub Title</th>
                        <th class="u-dt">Code</th>
                        
                    </tr>
                </thead>
                <tbody>

                    <tr v-for="(strategy,i) in strategies">
                        <td>
                            <nuxt-link  :to="{ path: '/clients/' + strategy.id}" >
                                {{++i}}
                            </nuxt-link>
                        </td>
                        <td class="u-dt"><small>{{strategy.title}}</small></td>
                        <td class="u-dt">{{strategy.sub_title}}<small>%pa</small></td>
                        <td class="u-dt">{{strategy.code}}<small>%</small></td>
                        <!-- <td><ticker :performance="strategy.ytd" /></td>
                        <td><ticker :performance="strategy.performance" /></td> -->
                    </tr>
                </tbody>
            </table>
        </part>

        <!-- <cookie-notice /> -->
        
    </div>

</template>

<script>
export default {
    // middleware: 'auth',
    async asyncData({$content, params }) {
        const page = await $content('/clients', params.report || "index").fetch()
        return { 
            page
        }
    },

    data() {
        return {
            strategies: [],
            token: '',
            sl : 1
        }
    },
    head() {
        return {
            title: this.page.title,
            meta: [
                { hid:"og:title", property:"og:title", content: 'Odin Clients' },
            ]
        }
    },
    mounted(){
        if(this.$auth.$storage.getUniversal('token'))
        {
            this.$auth.$storage.setUniversal('loggedIn', true)
            this.$auth.$storage.setUniversal('user', this.$auth.$storage.getUniversal('user'))
        }
    },
    created() {
        if(!this.$auth.$storage.getUniversal('loggedIn'))
        {
            this.$router.push('/login')
        }
        let token=this.$auth.$storage.getUniversal('token')
        this.$axios.setToken(token, 'Bearer')
        this.$axios
          .get("https://test-my.workstation.co.uk/api/webpages", {
          })
          .then((res) => {
            //console.log(res.data);
            this.strategies=res.data.data;
          })
          .catch(function (error) {
            console.log(error);
          });
        
    },
    methods: {
        inc(){
            this.sl=this.sl + 1;
            return this.sl;
        }
    }
}
</script>


<style lang="scss">

@import "~/assets/css/variables.css";

table {
    width: 94%;
    margin: 1em 3%;
}

th, td {
    text-align: center;
    padding: 0.75em 0.25em;
    margin: 0;
}

th:first-child, 
td:first-child {
    text-align: left;
}

th {
    border-bottom: 2px var(--default) solid;
}

td {
    border-bottom: 1px var(--default) solid;
}

.u-dt { display: none; }
@media (min-width: 768px) { .u-dt { display: table-cell; } }


</style>
