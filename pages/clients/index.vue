<template>
    <div>

        <part>
            <nuxt-content :document="page" />

            <table cellpadding="0" cellspacing="0" border="0">
                <thead>
                    <tr>
                        <th>Strategy</th>
                        <th class="u-dt">Inception</th>
                        <th class="u-dt">Target</th>
                        <th class="u-dt">Drawdown</th>
                        <th>YTD</th>
                        <th>Performance</th>
                    </tr>
                </thead>
                <tbody>
                    <tr v-for="strategy in strategies">
                        <td>
                            <nuxt-link :to="'/clients/' + strategy.url">
                                {{strategy.strategy}}
                            </nuxt-link>
                        </td>
                        <td class="u-dt"><small>{{strategy.inception}}</small></td>
                        <td class="u-dt">{{strategy.target}}<small>%pa</small></td>
                        <td class="u-dt"><{{strategy.drawdown}}<small>%</small></td>
                        <td><ticker :performance="strategy.ytd" /></td>
                        <td><ticker :performance="strategy.performance" /></td>
                    </tr>
                </tbody>
            </table>
        </part>

        <!-- <cookie-notice /> -->
        
    </div>

</template>

<script>
export default {
    async asyncData({$content, params }) {
        const page = await $content('/clients', params.report || "index").fetch()
        return { 
            page
        }
    },
    data() {
        return {
            strategies: [
                {
                    strategy: 'Systematic Equity ECM',
                    url: 'systematic-equity-ecm',
                    target: 25,
                    drawdown: 10,
                    ytd: 3,
                    performance: 15,
                    inception: 'Dec 2021'
                },
                {
                    strategy: 'Volatility Arbitrage',
                    url: 'volatility-arbitrage',
                    target: 25,
                    drawdown: 10,
                    ytd: 3,
                    performance: 15,
                    inception: 'Dec 2021'
                },
                {
                    strategy: 'Active Equity',
                    url: 'active-equity',
                    target: 25,
                    drawdown: 10,
                    ytd: -2.2,
                    performance: 9.3,
                    inception: 'Dec 2021'
                },
                {
                    strategy: 'Systematic Macro',
                    url: 'systematic-macro',
                    target: 25,
                    drawdown: 10,
                    ytd: 3.1,
                    performance: 15.8,
                    inception: 'Dec 2021'
                }
            ]
        }
    },
    head() {
        return {
            title: this.page.title,
            meta: [
                { hid:"og:title", property:"og:title", content: 'Odin Clients' },
            ]
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
