<template>
    <div>

        <part>
            <nuxt-content :document="page" />

            <!-- <pre>{{page}}</pre> -->
            <!-- <pre>{{strategies}}</pre> -->

            <table cellpadding="0" cellspacing="0" border="0">
                <thead>
                    <tr>
                        <th>Strategy</th>
                        <th class="u-dt">Inception</th>
                        <th v-for="(customfield, index) in customfields" :key="index">{{ customfield.field_name }}</th>
                    </tr>
                </thead>
                <tbody>

                    <tr v-for="(strategy, i) in strategies">
                        <td>
                            <nuxt-link :to="{ path: '/clients/' + strategy.id }">
                                {{ strategy.title }}
                            </nuxt-link>
                            <!-- <pre>{{strategy}}</pre> -->
                        </td>
                        <td class="u-dt"><small>{{ convertData(parseInt(strategy.publish_date)) }}</small></td>

                        <td v-for="(customfield, index) in customfields" :key="index">
                            <ticker
                                :performance="renderFieldValue(strategy.customFields[index] && strategy.customFields[index].field_value)" />
                        </td>
                    </tr>
                </tbody>
            </table>
        </part>

    </div>
</template>

<script>
export default {
    // middleware: 'auth',
    beforeRouteEnter(to, from, next) {
        if (!to.query.random) {
            next({ path: to.path, query: { random: Date.now() } });
        } else {
            next();
        }
    },
    async asyncData({ $content, params }) {
        const page = await $content('/clients', params.report || "index").fetch()
        return {
            page
        }
    },
    data() {
        return {
            strategies: [],
            token: '',
            sl: 1,
            customfields: [],
        }
    },
    head() {
        return {
            title: this.page.title,
            meta: [
                { hid: "og:title", property: "og:title", content: 'Odin Clients' },
            ]
        }
    },
    mounted() {
        if (this.$auth.$storage.getUniversal('token')) {
            this.$auth.$storage.setUniversal('loggedIn', true)
            this.$auth.$storage.setUniversal('user', this.$auth.$storage.getUniversal('user'))
        }
    },
    created() {
        let testUrl = (this.$config.baseAPIURL && this.$auth.$storage.getUniversal('user'))
            ? this.$config.baseAPIURL + "/webpages/" + this.$auth.$storage.getUniversal('user').client_id
            : 'null';
        // console.log("API URL: " + testUrl);


        if (!this.$auth.$storage.getUniversal('loggedIn')) {
            this.$router.push('/login');
        }
        let token = this.$auth.$storage.getUniversal('token')
        this.$axios.setToken(token, 'Bearer');
        this.$axios
            .get((this.$config.baseAPIURL && this.$auth.$storage.getUniversal('user'))
                ? this.$config.baseAPIURL + "/webpages/" + this.$auth.$storage.getUniversal('user').client_id
                : null, {
            })
            .then((res) => {
                const response = res.data.data;
                this.strategies = res.data.data;
                response.forEach((responseData, index) => {
                    if (responseData?.customFields?.length) {
                        this.customfields = responseData?.customFields;
                    }
                })
            })
            .catch((error) => {
                console.log({ error });
                this.$auth.$storage.setUniversal('token', null);
                this.$auth.$storage.setUniversal('loggedIn', false);
                this.$auth.$storage.setUniversal('user', null);
                this.$router.push('/login');
            });

    },
    methods: {
        inc() {
            this.sl = this.sl + 1;
            return this.sl;
        },
        convertData: function (timestamp) {
            const date = new Date(timestamp * 1000);
            return date.toLocaleDateString()
        },
        renderFieldValue: function (value) {
            if (!value) {
                return 0
            }
            return value
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

th,
td {
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

.u-dt {
    display: none;
}

@media (min-width: 768px) {
    .u-dt {
        display: table-cell;
    }
}
</style>
