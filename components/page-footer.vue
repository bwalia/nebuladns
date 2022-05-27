<template>
    <footer class="footer">
        <part>

            <div class="u-display-f">
                <logo style="margin-left: 3%;" />
            </div>

            <div class="legals">
                <nav>
                    <!-- <nuxt-link to="/terms-conditions">Terms & Conditions</nuxt-link><span>&nbsp;&nbsp;|&nbsp;&nbsp;</span> -->
                    <!-- <nuxt-link to="/privacy-policy">Privacy Policy</nuxt-link><span>&nbsp;&nbsp;|&nbsp;&nbsp;</span> -->
                    <nuxt-link to="/cookie-policy">Cookie Policy</nuxt-link>                
                </nav>    

                <small>
                    Odin Capital Management Ltd is Authorised and Regulated by the Financial Conduct Authority no. 478321.
                </small>
            </div>
           
        </part>
    </footer>
</template>

<style lang="scss" scoped>

@import "~/assets/css/variables.css";
@import "~/assets/css/scale.scss";

.footer {
    min-height: 140px;
    background: var(--tint);
    border-top: 2px solid var(--default);
}

.footer a,
.footer span,
.footer small {
    color: var(--dark);
    font-size: $scale-min;
}

.legals {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: flex-start;
    line-height: 24px;
    margin: 0 0 0 30px;
    padding: 0;
    @media (min-width: 768px) { 
        flex-wrap: nowrap;
    }
}

.legals > * {
    flex: 0 0 48%
}

</style>